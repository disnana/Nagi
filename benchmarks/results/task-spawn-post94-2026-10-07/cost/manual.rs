pub async fn work(value: i64) -> i64 {
    value
}
pub async fn receive_batch(n: i64) -> Result<i64, nagi_runtime::Error> {
    let mut total = 0;
    {
        let mut scope = nagi_runtime::TaskScope::new();
        let body: Result<(), nagi_runtime::Error> = {
            for number in 0..n {
                let future = work(number);
                let child = scope.spawn_value(async move { future.await });
                let received = scope.receive(child).await;
                match received {
                    Ok(value) => total += value,
                    Err(_failure) => assert!(false),
                }
            }
            Ok(())
        };
        if let Err(error) = body {
            let _ = scope.cancel().await;
            return Err(error);
        }
        scope.join().await?;
    }
    Ok(total)
}
pub async fn discard_batch(n: i64) -> Result<i64, nagi_runtime::Error> {
    {
        let mut scope = nagi_runtime::TaskScope::new();
        let body: Result<(), nagi_runtime::Error> = {
            for number in 0..n {
                let future = work(number);
                let child = scope.spawn_value(async move { future.await });
                scope.discard(child);
            }
            Ok(())
        };
        if let Err(error) = body {
            let _ = scope.cancel().await;
            return Err(error);
        }
        scope.join().await?;
    }
    Ok(0)
}
