#[macro_use]
extern crate error_chain;
extern crate futures;
extern crate futures_cpupool;

#[macro_use]
extern crate log;
extern crate log4rs;

#[macro_use]
extern crate serde_derive;
extern crate structopt;
extern crate subprocess;

#[macro_use]
extern crate structopt_derive;
extern crate toml;

use futures::Future;
use futures_cpupool::CpuPool;
use std::fs::File;
use std::io::{self, Read, Write};
use std::process;
use subprocess::{Exec, ExitStatus};
use structopt::StructOpt;

mod errors {
    error_chain! {
        errors {
        }
    }
}

use errors::*;

#[derive(Serialize, Deserialize, Debug)]
struct FileConfig {
    processes: Vec<String>,
}

#[derive(StructOpt, Debug)]
#[structopt(name = "Batch-Exec", about = "Program to launch all the processes and wait for their exit code")]
struct ArgConfig {
    #[structopt(short = "c", long = "config", help = "File configuration path")]
    config_path: String,

    #[structopt(short = "l", long = "log-config", help = "Log configuration file path")]
    log_config_path: String,
}

fn run() -> Result<()> {
    let arg_config = ArgConfig::from_args();

    log4rs::init_file(&arg_config.log_config_path, Default::default())
       .chain_err(|| format!("Unable to initialize log4rs logger with the given config file at '{}'", arg_config.log_config_path))?;

    let config_str = {
        let mut config_file = File::open(&arg_config.config_path)
            .chain_err(|| format!("Unable to open config file path at {:?}", arg_config.config_path))?;

        let mut s = String::new();

        config_file.read_to_string(&mut s)
            .map(|_| s)
            .chain_err(|| "Unable to read config file into string")?
    };

    let config: FileConfig = toml::from_str(&config_str)
        .chain_err(|| format!("Unable to parse config as required toml format: {}", config_str))?;

    info!("Completed configuration initialization!");

    // write the body here
    
    // starts launching of processes 
    let pool = CpuPool::new(config.processes.len()); 

    let process_res_futs: Vec<_> = config.processes.iter().cloned()
        .map(|process| {
            pool.spawn_fn(move || {
                let process_str = process.clone();

                let process_run = move || -> Result<ExitStatus> {
                    // process thread body
                    let exit_status = Exec::shell(process)
                        .join()
                        .chain_err(|| format!("Unable to join shell process"))?;

                    Ok(exit_status)
                };

                let process_res = process_run();

                match process_res {
                    Ok(ref exit_status) => info!("Shell terminated [{}], exit code: {:?}", process_str, exit_status),
                    Err(ref e) => error!("Shell error [{}]: {}", process_str, e),
                }

                process_res
            })
        })
        .collect();

    for process_res_fut in process_res_futs.into_iter() {
        let res = process_res_fut.wait();

        if let Err(e) = res {
            println!("Error waiting for child future: {}", e);
        }
    }

    Ok(())
}

fn main() {
    match run() {
        Ok(_) => {
            println!("Program completed!");
            process::exit(0)
        },

        Err(ref e) => {
            let stderr = &mut io::stderr();

            writeln!(stderr, "Error: {}", e)
                .expect("Unable to write error into stderr!");

            for e in e.iter().skip(1) {
                writeln!(stderr, "- Caused by: {}", e)
                    .expect("Unable to write error causes into stderr!");
            }

            process::exit(1);
        },
    }
}
