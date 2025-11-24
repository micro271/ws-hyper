#!/bin/bash

docker run -d -e POSTGRES_PASSWORD=admin -e POSTGRES_USER=test -e POSTGRES_DB=buckets -v ./initdb.sql:/docker-entrypoint-initdb.d/init.sql -p 5432:5432 --name postgres postgres