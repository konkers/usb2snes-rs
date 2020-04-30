use async_std;
use failure::{format_err, Error};
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use std::time::Instant;
use structopt::StructOpt;
use usb2snes::{Connection, FileType};

#[derive(StructOpt)]
enum Command {
    Info,
    Ls {
        path: Option<String>,
    },
    Put {
        #[structopt(long)]
        dest_dir: Option<String>,

        #[structopt(parse(from_os_str))]
        files: Vec<PathBuf>,
    },
    Rm {
        files: Vec<String>,
    },
}

#[derive(StructOpt)]
struct Opt {
    #[structopt(long)]
    device: Option<String>,

    #[structopt(subcommand)]
    cmd: Command,
}

async fn handle_info(c: &mut Connection) -> Result<(), Error> {
    for info in c.get_info().await? {
        println!(" * {}", info);
    }
    Ok(())
}

async fn handle_ls(c: &mut Connection, path: Option<String>) -> Result<(), Error> {
    let path = path.unwrap_or("".to_string());
    for fi in c.list_files(&path).await? {
        println!(
            "{}{}",
            fi.name,
            if fi.ty == FileType::Dir { "/" } else { "" }
        );
    }

    Ok(())
}

async fn handle_put(
    c: &mut Connection,
    dest_dir: Option<String>,
    files: Vec<PathBuf>,
) -> Result<(), Error> {
    // let dest_dir = dest_dir.unwrap_or("".to_string()).trim_end_matches('/');

    for path in files {
        let mut f = File::open(&path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        let file_name = path
            .file_name()
            .ok_or(format_err!("can't parse file name from {:?}", path))?
            .to_string_lossy()
            .to_string();

        let remote_name = match &dest_dir {
            None => file_name,
            Some(d) => format!("{}/{}", &d.trim_end_matches('/'), file_name),
        };

        println!("{} -> {}", &path.to_string_lossy(), &remote_name);
        c.put_file(&remote_name, &buf).await?;

        // Hack to wait til file is transferred.
        c.list_files("").await?;
    }
    Ok(())
}

async fn handle_rm(c: &mut Connection, files: Vec<String>) -> Result<(), Error> {
    for path in files {
        println!("removing {}", &path);
        c.rm(&path).await?;
    }
    Ok(())
}

async fn run(opt: Opt) -> Result<(), Error> {
    let mut c = usb2snes::Connection::new("ws://localhost:8080").await?;

    let dev = match opt.device {
        Some(d) => d,
        None => {
            let devs = c.get_device_list().await?;
            devs[0].to_string()
        }
    };

    println!("Attaching to {}.", dev);
    c.attach(&dev).await?;

    match opt.cmd {
        Command::Info => handle_info(&mut c).await?,
        Command::Ls { path } => handle_ls(&mut c, path).await?,
        Command::Put { dest_dir, files } => handle_put(&mut c, dest_dir, files).await?,
        Command::Rm { files } => handle_rm(&mut c, files).await?,
    };

    if false {
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
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();
    async_std::task::block_on(run(opt))?;
    Ok(())
}
