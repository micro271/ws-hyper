use futures::StreamExt;
use mongodb::bson::{Document, doc};
use std::collections::HashSet;

use crate::{
    bucket::{
        Bucket,
        bucket_map::{BucketMap, BucketMapType},
        key::Key,
    },
    state::local_storage::{AsObjectDeserialize, COLLECTION, LocalStorage},
};

pub async fn sync_object_to_database(local_storage: &LocalStorage, map: &BucketMap) {
    let db = local_storage.raw();
    let mut cursor = db
        .collection::<AsObjectDeserialize>(COLLECTION)
        .find(doc! {})
        .await
        .unwrap();
    let mut tree_aux: BucketMapType = Default::default();

    loop {
        match cursor.next().await {
            Some(Ok(item)) => {
                let tmp = tree_aux
                    .entry(Bucket::new_unchecked(item.bucket))
                    .or_default();
                let tmp = tmp.entry(Key::new(item.key)).or_default();
                tmp.push(item.object);
            }
            Some(Err(er)) => tracing::error!("{er}"),
            None => break,
        }
    }
    let bucket_map = map.keys().cloned().collect::<HashSet<_>>();
    let bucket_db = tree_aux.keys().cloned().collect::<HashSet<_>>();
    let dif = bucket_db.difference(&bucket_map).collect::<HashSet<_>>();

    tracing::warn!(
        "[ fn_sync_object_to_database ] {{ Inconsistensy between mongodb and fs }} difference: {dif:#?}"
    );

    for i in dif {
        if let Err(er) = db
            .collection::<Document>(COLLECTION)
            .delete_many(doc! {"bucket": i.name()})
            .await
        {
            tracing::error!("[LocalStorage] {{ Delete Bucket in Db }} Error: {er}");
        }
        let branch = tree_aux.remove(i);
        tracing::warn!("[LocalStorage] {{ Delete Branch }} bucket: {branch:#?}");
    }

    for i in bucket_db.intersection(&bucket_map).collect::<HashSet<_>>() {
        let key_map = map
            .get(i)
            .unwrap()
            .keys()
            .cloned()
            .collect::<HashSet<Key>>();
        let key_db = tree_aux
            .get(i)
            .unwrap()
            .keys()
            .cloned()
            .collect::<HashSet<Key>>();
        let dif = key_db.difference(&key_map).collect::<HashSet<_>>();
        for j in dif {
            if let Err(er) = db
                .collection::<Document>(COLLECTION)
                .delete_many(doc! {"bucket": i.name(), "key": j.name()})
                .await
            {
                tracing::error!("[LocalStorage] {{ Delete Bucket in Db }} Error: {er}");
            }
            let branch = tree_aux.get_mut(i).unwrap();
            branch.remove(j);
            tracing::warn!("[LocalStorage] {{ Delete Branch }} key: {branch:#?}");
        }

        for m in key_db.intersection(&key_map).collect::<HashSet<_>>() {
            let vec_map = map.get(i).and_then(|x| x.get(m)).unwrap();
            let vec_db = tree_aux.get(i).and_then(|x| x.get(m)).unwrap();
            let to_delete = vec_db
                .iter()
                .filter(|x| !vec_map.iter().any(|y| y.chechsum == x.chechsum))
                .collect::<Vec<_>>();
            for n in to_delete {
                if let Err(er) = db
                    .collection::<Document>(COLLECTION)
                    .delete_one(doc! {"bucket": i.name(), "key": m.name(), "object.name": &n.name})
                    .await
                {
                    tracing::error!("[LocalStorage] {{ Delete Object in Db }} Error: {er}");
                } else {
                    tracing::warn!(
                        "[LocalStorage] {{ Delete Object in Db }} bucket: {i}, key: {m:?}, object.name: {:?}",
                        n.name
                    );
                }
            }
        }
    }
}
