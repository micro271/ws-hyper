pub mod builder;

pub struct LogLayer<OnReq, OnRes, N> {
    on_req: OnReq,
    on_res: OnRes,
    next: N
}