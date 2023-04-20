
#[shuttle_runtime::main]
async fn shuttle_main() -> Result<MyService, shuttle_runtime::Error> {
    Ok(MyService {})
}

struct MyService {}

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for MyService {
    async fn bind(self, _addr: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        Ok(())
    }
}
