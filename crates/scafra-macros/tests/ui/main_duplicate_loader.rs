mod app {
    #[scafra_macros::main(config_loader = first, config_loader = second)]
    async fn main() {}
}

fn main() {}
