use crate::flutter::attach::{Event, VmService};
use anyhow::Result;
use console::Term;
use std::path::PathBuf;

pub async fn attach(url: &str, root_dir: PathBuf, target_file: PathBuf) -> Result<()> {
    let term = Term::stdout();
    let mut vm = VmService::attach(url, root_dir, target_file).await?;
    let mut vmservice = None;
    let mut devtool = None;
    while vmservice.is_none() || devtool.is_none() {
        match vm.next_event().await? {
            Event::VmServiceUrl(value) => {
                vmservice = Some(value);
            }
            Event::DevToolsAddress(value) => {
                devtool = Some(value);
            }
        }
    }
    let vmservice = vmservice.unwrap();
    let devtool = devtool.unwrap();

    println!("Dart Observatory {}", vmservice);
    println!("Flutter DevTools {}?uri={}", devtool, vmservice);
    println!("");
    println!("Flutter run key commands.");
    println!("r Hot reload. 🔥🔥🔥");
    println!("R Hot restart.");
    println!("d Detach (terminate \"x run\" but leave application running).");
    println!("q Quit (terminate the application on the device).");
    println!("");

    loop {
        match term.read_char()? {
            'r' => {
                println!("Performing hot reload...");
                vm.hot_reload().await?;
                //println!("Reloaded {} libraries in {}ms.");
            }
            'R' => {
                println!("Performing hot restart...");
                vm.hot_restart().await?;
                //println!("Restarted application in {}ms.");
            }
            'd' => break,
            'q' => {
                vm.quit().await?;
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
