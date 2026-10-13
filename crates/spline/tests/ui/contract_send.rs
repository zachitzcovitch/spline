#[spline::contract(namespace = "users")]
trait Users {
    #[query]
    async fn get(&self) -> Result<(), ()>;
}

struct Server;
impl Users for Server {
    async fn get(&self) -> Result<(), ()> {
        let local = std::rc::Rc::new(());
        std::future::ready(()).await;
        drop(local);
        Ok(())
    }
}

fn main() {}
