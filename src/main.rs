use async_std;
use failure::Error;
use std::fs::File;
use std::io::prelude::*;
use std::time::Instant;

async fn run() -> Result<(), Error> {
    let mut c = usb2snes::Connection::new("ws://localhost:8080").await?;
    let devs = c.get_device_list().await?;
    println!("devs: {:?}", devs);
    c.attach(&devs[0]).await?;

    let info = c.get_info().await?;
    println!("info: {:?}", info);

    let files = c.list_files("").await?;
    println!("files: {:?}", files);

    let start = Instant::now();
    let mut f = File::open("ff4.smc")?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    c.put_file("ff4fe/ff4.smc", &buf).await?;

    let files = c.list_files("").await?;
    println!("files:; {:?}", files);

    println!("elapsed: {:?}", start.elapsed());

    c.close().await?;
    Ok(())
}

fn main() -> Result<(), Error> {
    async_std::task::block_on(run())?;
    Ok(())
}
