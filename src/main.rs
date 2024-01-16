use async_timer::{
    new_spawner_executor,
    timer::TimerFuture,
    Executor,
    Spawner
};

use std::time::Duration;
    

fn main() {
    let (spawner, executor): (Spawner, Executor) = new_spawner_executor();

    spawner.spawn(async {
        println!("Hello from custom spawner");
        TimerFuture::new(Duration::from_secs(2)).await;
        println!("Exiting from custom spawner");
    });

    drop(spawner);

    executor.run();
}
