use async_std;
use byteorder::{LittleEndian, ReadBytesExt};
use failure::{format_err, Error};
use num::{FromPrimitive, ToPrimitive};
use num_derive::{FromPrimitive, ToPrimitive};
use parse_int::parse;
use std::fs::File;
use std::io::prelude::*;
use std::io::Cursor;
use std::num::ParseIntError;
use std::path::PathBuf;
use structopt::StructOpt;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use usb2snes::{Connection, FileType};

#[derive(Debug, EnumIter, FromPrimitive, ToPrimitive)]
enum KeyItem {
    Package = 0x00,
    SandRuby = 0x01,
    LegendSword = 0x02,
    BaronKey = 0x03,
    TwinHarp = 0x04,
    EarthCrystal = 0x05,
    MagmaKey = 0x06,
    TowerKey = 0x07,
    Hook = 0x08,
    LucaKey = 0x09,
    DarknessCrystal = 0x0a,
    RatTail = 0x0b,
    Adamant = 0x0c,
    Pan = 0x0d,
    Spoon = 0x0e,
    PinkTail = 0x0f,
    Crystal = 0x10,
}

#[repr(u16)]
#[derive(Debug, FromPrimitive, ToPrimitive)]
enum Location {
    StartingItem = 0x20,
    Antlion = 0x21,
    DefendingFabul = 0x22,
    MtOrdeals = 0x23,
    BaronInn = 0x24,
    BaronCastle = 0x25,
    EdwardInToroia = 0x26,
    CaveMagnes = 0x27,
    TowerOfZot = 0x28,
    LowerBabIlBoss = 0x29,
    SuperCannon = 0x2a,
    Luca = 0x2b, // aka DwarfCastle
    SealedCave = 0x2c,
    FeymarchChest = 0x2d,
    RatTail = 0x2e,
    YangsWife = 0x2f,
    YangsWifePan = 0x30,
    FeymarchQueen = 0x31,
    FeymarchKing = 0x32,
    Odin = 0x33,
    Sylphs = 0x34,
    CaveBahamut = 0x35,
    PaleDim = 0x36,
    Wyvern = 0x37,
    Plauge = 0x38,
    DLunar1 = 0x39,
    DLunar2 = 0x3a,
    Ogopogo = 0x3b,
    TowerOfZotTrappedChest = 0x3c,
    EblanTrappedChest1 = 0x3d,
    EblanTrappedChest2 = 0x3e,
    EblanTrappedChest3 = 0x3f,
    LowerBabIlTappedChest1 = 0x40,
    LowerBabIlTappedChest2 = 0x41,
    LowerBabIlTappedChest3 = 0x42,
    LowerBabIlTappedChest4 = 0x43,
    CaveEblanTrappedChest = 0x44,
    UpperBabIlTrappedChest = 0x45,
    CaveOfSummonsTrappedChest = 0x46,
    SyplhCaveTrappedChest1 = 0x47,
    SyplhCaveTrappedChest2 = 0x48,
    SyplhCaveTrappedChest3 = 0x49,
    SyplhCaveTrappedChest4 = 0x4a,
    SyplhCaveTrappedChest5 = 0x4b,
    SyplhCaveTrappedChest6 = 0x4c,
    SyplhCaveTrappedChest7 = 0x4d,
    GiantOfBabIlTrappedChest = 0x4e,
    LunarPathTrappedChest = 0x4f,
    LunarCoreTrappedChest1 = 0x50,
    LunarCoreTrappedChest2 = 0x51,
    LunarCoreTrappedChest3 = 0x52,
    LunarCoreTrappedChest4 = 0x53,
    LunarCoreTrappedChest5 = 0x54,
    LunarCoreTrappedChest6 = 0x55,
    LunarCoreTrappedChest7 = 0x56,
    LunarCoreTrappedChest8 = 0x57,
    LunarCoreTrappedChest9 = 0x58,
    RydiasMom = 0x59,
    FallenGolbez = 0x5a,
    ObjectiveCompletion = 0x5d,
}

fn parse_num(src: &str) -> Result<u32, ParseIntError> {
    parse::<u32>(src)
}

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
    Read {
        #[structopt(parse(try_from_str = parse_num))]
        addr: u32,

        #[structopt(parse(try_from_str = parse_num))]
        len: u32,
    },
    Flags,
    Track,
}

#[derive(StructOpt)]
struct Opt {
    #[structopt(long)]
    device: Option<String>,

    #[structopt(subcommand)]
    cmd: Command,
}

async fn handle_flags(c: &mut Connection) -> Result<(), Error> {
    let mut buf = [0; 4];
    c.read_mem(0x1ff000, &mut buf).await?;
    let flags_len = Cursor::new(buf).read_u32::<LittleEndian>()?;

    let mut flags_buf = vec![0; flags_len as usize];
    c.read_mem(0x1ff004, &mut flags_buf).await?;
    let flags = String::from_utf8(flags_buf)?;
    println!("{}", flags);
    Ok(())
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

async fn handle_read(c: &mut Connection, addr: u32, len: u32) -> Result<(), Error> {
    let mut data = vec![0; len as usize];
    c.read_mem(addr, &mut data).await?;
    println!("{:?}", data);
    Ok(())
}

async fn handle_track(c: &mut Connection) -> Result<(), Error> {
    for ki in KeyItem::iter() {
        let mut buf = [0; 2];
        let index = ki.to_u32().unwrap();
        c.read_mem(0xe07080 + 2 * index, &mut buf).await?;
        let loc_val = Cursor::new(buf).read_u16::<LittleEndian>()?;
        let loc = Location::from_u16(loc_val);
        let loc_str = match loc {
            Some(l) => format!("{:?}", l),
            None => "".to_string(),
        };
        println!("{:?} = {}", ki, loc_str);
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
        Command::Flags => handle_flags(&mut c).await?,
        Command::Info => handle_info(&mut c).await?,
        Command::Ls { path } => handle_ls(&mut c, path).await?,
        Command::Put { dest_dir, files } => handle_put(&mut c, dest_dir, files).await?,
        Command::Rm { files } => handle_rm(&mut c, files).await?,
        Command::Read { addr, len } => handle_read(&mut c, addr, len).await?,
        Command::Track => handle_track(&mut c).await?,
    };

    Ok(())
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();
    async_std::task::block_on(run(opt))?;
    Ok(())
}
