use clap::Parser;
use osb_sim::{Scenario,Simulation,TICKS_PER_SECOND};
use std::{error::Error,fs,io::{BufWriter,Write},path::PathBuf,time::Instant};
mod benchmark;

#[derive(Parser)]
#[command(version,about="OpenStreetBattle deterministic headless simulation. No display, GPU or API key required.")]
struct Args {
    /// Scenario JSON. Omit to use the embedded First Contact scenario.
    scenario:Option<PathBuf>,
    #[arg(long,conflicts_with_all=["scenario","map","seed"])]
    load:Option<PathBuf>,
    /// Existing small OSM/Overpass JSON export. No network requests are made.
    #[arg(long,conflicts_with="scenario")]
    map:Option<PathBuf>,
    #[arg(long)] seed:Option<u64>,
    /// Additional simulated duration, integer plus s/m/h/d/w.
    #[arg(long,default_value="2m",value_parser=parse_duration)]
    duration:u64,
    #[arg(long)] save:Option<PathBuf>,
    /// Stream all new events as JSON Lines while running.
    #[arg(long)] events:Option<PathBuf>,
    #[arg(long)] write_demo:Option<PathBuf>,
    /// Dispersed, no-contact storage/update probes. NOT a campaign-scale combat benchmark.
    #[arg(long)] benchmark:bool,
}
fn parse_duration(text:&str)->Result<u64,String> {
    let split=text.find(|c:char|!c.is_ascii_digit()).unwrap_or(text.len());
    if split==0 {return Err("duration must start with a non-negative integer".into());}
    let number=text[..split].parse::<u64>().map_err(|_|"duration integer overflow")?;
    let multiplier=match &text[split..] {""|"s"=>1,"m"=>60,"h"=>3600,"d"=>86400,"w"=>604800,_=>return Err("use seconds, minutes, hours, days or weeks: 30s, 2m, 1h, 7d, 1w".into())};
    number.checked_mul(multiplier).and_then(|s|s.checked_mul(TICKS_PER_SECOND)).ok_or_else(||"duration overflows simulation clock".into())
}
fn main()->Result<(),Box<dyn Error>> {
    let args=Args::parse();
    if let Some(path)=args.write_demo {fs::write(path,serde_json::to_string_pretty(&Scenario::demo())?)?;return Ok(());}
    if args.benchmark {return benchmark::run();}
    let mut sim=if let Some(path)=args.load {Simulation::restore(osb_campaign::load(path)?)?} else {
        let scenario=if let Some(path)=args.scenario {Scenario::parse(&read_bounded(&path,64*1024*1024)?)?} else if let Some(path)=args.map {Scenario::on_map(osb_world::import_osm_json(&read_bounded(&path,32*1024*1024)?)?)?} else {Scenario::demo()};
        Simulation::new(scenario,args.seed.unwrap_or(42))?
    };
    let mut event_output=args.events.map(fs::File::create).transpose()?.map(BufWriter::new);
    let mut last_event=sim.events().back().map_or(0,|e|e.sequence);
    let started=Instant::now();let initial_tick=sim.tick();
    for _ in 0..args.duration {
        sim.step()?;
        if let Some(output)=event_output.as_mut() {for e in sim.events() {if e.sequence>last_event {serde_json::to_writer(&mut *output,e)?;writeln!(output)?;last_event=e.sequence;}}}
    }
    if let Some(output)=event_output.as_mut() {output.flush()?;}
    if let Some(path)=args.save {
        if let Some(parent)=path.parent().filter(|p|!p.as_os_str().is_empty()) {fs::create_dir_all(parent)?;}
        osb_campaign::save(path,&sim.snapshot())?;
    }
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({
        "scenario":sim.scenario().name,"initial_tick":initial_tick,"tick":sim.tick(),
        "simulated_seconds":sim.tick() as f64/TICKS_PER_SECOND as f64,
        "elapsed_wall_seconds":started.elapsed().as_secs_f64(),"state_hash":sim.fingerprint()?,
        "forces":sim.statistics(),"recent_events":sim.events().iter().rev().take(12).collect::<Vec<_>>(),
        "limitations":"Prototype combat, bounded 2D map. No strategic campaign or simulation LOD yet."
    }))?);
    Ok(())
}
fn read_bounded(path:&std::path::Path,max:usize)->Result<String,Box<dyn Error>> {
    use std::io::Read;
    let mut text=String::new();fs::File::open(path)?.take((max+1) as u64).read_to_string(&mut text)?;
    if text.len()>max {return Err("input file exceeds size limit".into());}Ok(text)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn durations(){assert_eq!(parse_duration("2h").unwrap(),72000);assert_eq!(parse_duration("0s").unwrap(),0);assert_eq!(parse_duration("1w").unwrap(),6048000);}
    #[test] fn bad_duration(){for s in ["-1h","1.5h","forever","999999999999999999999d",""] {assert!(parse_duration(s).is_err());}}
}
