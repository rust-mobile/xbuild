use anyhow::{Context, Result};
use console::{style, Term};
use std::process::Command;
use std::time::Instant;

pub struct TaskRunner {
    term: Term,
    num_tasks: u32,
    current_task: u32,
    now: Instant,
    descr: String,
    verbose: bool,
    started: bool,
}

impl TaskRunner {
    pub fn new(num_tasks: u32, verbose: bool) -> Self {
        Self {
            term: Term::stdout(),
            num_tasks,
            current_task: 0,
            now: Instant::now(),
            descr: "".into(),
            verbose,
            started: false,
        }
    }

    fn task_id(&self) -> String {
        style(format!("[{}/{}]", self.current_task + 1, self.num_tasks))
            .force_styling(true)
            .to_string()
    }

    pub fn start_task(&mut self, descr: impl Into<String>) {
        if self.started {
            self.finish_task(true, true);
        }
        self.now = Instant::now();
        self.descr = descr.into();
        self.started = true;
        println!("{} {}", self.task_id(), &self.descr);
    }

    fn finish_task(&mut self, skipped: bool, clear_last: bool) {
        self.started = false;
        if clear_last {
            self.term.clear_last_lines(1).unwrap();
        }
        let status = if skipped {
            "[SKIPPED]".to_string()
        } else {
            let time = self.now.elapsed();
            format!("[{}ms]", time.as_millis())
        };
        println!("{} {} {}", self.task_id(), &self.descr, status,);
        self.current_task += 1;
    }

    pub fn end_task(&mut self) {
        self.finish_task(false, !self.verbose);
    }

    pub fn end_verbose_task(&mut self) {
        self.finish_task(false, false);
    }
}

pub fn run(command: &mut Command) -> Result<()> {
    fn format_error(command: &Command, status: Option<i32>) -> String {
        let status = if let Some(code) = status {
            format!(" exited with {}", code)
        } else {
            Default::default()
        };
        format!("{} `{:?}`{}", style("[ERROR]").red(), command, status)
    }
    let status = command
        .status()
        .with_context(|| format_error(command, None))?;
    if !status.success() {
        anyhow::bail!("{}", format_error(command, status.code()));
    }
    Ok(())
}
