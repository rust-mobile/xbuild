use anyhow::Result;
use std::io::Cursor;
use xmsix::pri::PriFile;

const PRI_BYTES: &'static [u8] = include_bytes!("resources.pri");

#[test]
fn test_read() -> Result<()> {
    let mut cursor = Cursor::new(PRI_BYTES);
    let pri = PriFile::read(&mut cursor)?;
    println!("{:#?}", pri);
    assert!(false);
    Ok(())
}
