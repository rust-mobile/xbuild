use crate::flutter::attach::{Event, VmService};
use anyhow::Result;
use console::Term;
use futures::StreamExt;
use std::path::Path;

pub async fn attach(
    url: &str,
    root_dir: &Path,
    target_file: &Path,
    _host_vmservice_port: Option<u16>,
) -> Result<()> {
    let term = Term::stdout();
    term.write_line(&format!("attaching to {}", url))?;
    let vm = VmService::attach(url, root_dir.into(), target_file.into()).await?;
    let mut events = vm.events().await?;
    let term2 = term.clone();
    tokio::spawn(async move {
        let mut vmservice = None;
        while let Some(event) = events.next().await {
            match event {
                Ok(Event::Error(error)) => {
                    term2.write_line(error.trim()).ok();
                }
                Ok(Event::VmServiceUrl(value)) => {
                    term2
                        .write_line(&format!("Dart Observatory {}", value))
                        .ok();
                    vmservice = Some(value);
                }
                Ok(Event::DevToolsAddress(value)) => {
                    if let Some(vmservice) = vmservice.as_ref() {
                        term2
                            .write_line(&format!("Flutter DevTools {}?uri={}", value, vmservice))
                            .ok();
                    }
                }
                Err(error) => {
                    tracing::error!("{}", error);
                }
            }
        }
    });

    print_header(&term)?;

    loop {
        match term.read_char()? {
            'r' => {
                term.write_line("Performing hot reload...")?;
                vm.reassemble().await?;
                //vm.hot_reload().await?;
                //println!("Reloaded {} libraries in {}ms.");
            }
            'R' => {
                term.write_line("Performing hot restart...")?;
                vm.hot_restart().await?;
                //println!("Restarted application in {}ms.");
            }
            'c' => {
                term.clear_screen()?;
                print_header(&term)?;
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

fn print_header(term: &Term) -> Result<()> {
    term.write_line("Flutter run key commands.")?;
    term.write_line("r Hot reload. 🔥🔥🔥")?;
    term.write_line("R Hot restart.")?;
    term.write_line("c Clear the screen.")?;
    term.write_line("d Detach (terminate \"x run\" but leave application running).")?;
    term.write_line("q Quit (terminate the application on the device).")?;
    term.write_line("")?;
    Ok(())
}
