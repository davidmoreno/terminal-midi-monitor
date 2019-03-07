/**
 *  Terminal MIDI Monitor -- Shows MIDI Events on the terminal
 *  Copyright (C) 2019 David Moreno / Coralbits SL <dmoreno@coralbits.com>
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/
#[macro_use]
extern crate lazy_static;
extern crate clap;
extern crate alsa;
extern crate libc;

use alsa::seq;
use std::error;
use std::ffi::CString;
use colored::*;
use std::collections::HashMap;
use std::time::Instant;
use clap::{Arg, App};
use std::io;
use std::io::prelude::*;

lazy_static! {
    static ref CC_MAP: HashMap<u32, String> = build_cc_map();
}
const BPM_DAMPING: f64 = 0.03;

struct MidiMonitor<'a> {
    start_time: Instant,
    seq: &'a seq::Seq,
    last_clock: f64,
    average_sec_per_clock: f64,  // Rolling average
    clock_pos: i32, // Song position. once per clock.
    autoconnect: bool, // Whether to autoconnect to new ports
    port: i32,
    port_names: HashMap<seq::Addr, String>,
}

// List from http://nickfever.com/music/midi-cc-list
fn build_cc_map() -> HashMap<u32, String> {
    [
        (0, "Bank Select".to_string()),
        (1, "Modulation".to_string()),
        (2, "Breath Controller".to_string()),
        (3, "Undefined".to_string()),
        (4, "Foot Controller".to_string()),
        (5, "Portamento Time".to_string()),
        (6, "Data Entry Most Significant Bit(MSB)".to_string()),
        (7, "Volume".to_string()),
        (8, "Balance".to_string()),
        (9, "Undefined".to_string()),
        (10, "Pan".to_string()),
        (11, "Expression".to_string()),
        (12, "Effect Controller 1".to_string()),
        (13, "Effect Controller 2".to_string()),
        // (14, "Undefined".to_string()),
        // (15, "Undefined".to_string()),
        //(1, "General Purpose".to_string()),
        //(1, "Undefined".to_string()),
        // (1, "Controller 0-31 Least Significant Bit (LSB)".to_string()),
        (64, "Damper Pedal / Sustain Pedal".to_string()),
        (65, "Portamento On/Off Switch".to_string()),
        (66, "Sostenuto On/Off Switch".to_string()),
        (67, "Soft Pedal On/Off Switch".to_string()),
        (68, "Legato FootSwitch".to_string()),
        (69, "Hold 2".to_string()),
        (70, "Sound Controller 1".to_string()),
        (71, "Sound Controller 2".to_string()),
        (72, "Sound Controller 3".to_string()),
        (73, "Sound Controller 4".to_string()),
        (74, "Sound Controller 5".to_string()),
        (75, "Sound Controller 6".to_string()),
        (76, "Sound Controller 7".to_string()),
        (77, "Sound Controller 8".to_string()),
        (78, "Sound Controller 9".to_string()),
        (79, "Sound Controller 10".to_string()),
        (80, "General Purpose MIDI CC Controller".to_string()),
        (81, "General Purpose MIDI CC Controller".to_string()),
        (82, "General Purpose MIDI CC Controller".to_string()),
        (83, "General Purpose MIDI CC Controller".to_string()),
        (84, "Portamento CC Control".to_string()),
        // (, "Undefined".to_string()),
        (91, "Effect 1 Depth".to_string()),
        (92, "Effect 2 Depth".to_string()),
        (93, "Effect 3 Depth".to_string()),
        (94, "Effect 4 Depth".to_string()),
        (95, "Effect 5 Depth".to_string()),
        (96, "(+1) Data Increment".to_string()),
        (97, "(-1) Data Decrement".to_string()),
        (98, "Non-Registered Parameter Number LSB (NRPN)".to_string()),
        (99, "Non-Registered Parameter Number MSB (NRPN)".to_string()),
        (100, "Registered Parameter Number LSB (RPN)".to_string()),
        (101, "Registered Parameter Number MSB (RPN)".to_string()),
        // (1, "Undefined".to_string()),
        // (1, "".to_string()),
        (120, "All Sound Off".to_string()),
        (121, "Reset All Controllers".to_string()),
        (122, "Local On/Off Switch".to_string()),
        (123, "All Notes Off".to_string()),
        (124, "Omni Mode Off".to_string()),
        (125, "Omni Mode On".to_string()),
        (126, "Mono Mode".to_string()),
        (127, "Poly Mode".to_string()),
    ].iter().cloned().collect()
}


fn setup_alsaseq() -> Result<(seq::Seq, i32), Box<error::Error>>{
    let seq = seq::Seq::open(None, Some(alsa::Direction::Capture), true)?;
    seq.set_client_name(&CString::new("Terminal MIDI Monitor")?)?;

    let mut dinfo = seq::PortInfo::empty()?;
    dinfo.set_capability(seq::WRITE | seq::SUBS_WRITE);
    dinfo.set_type(seq::MIDI_GENERIC | seq::APPLICATION);
    dinfo.set_name(&CString::new("Input")?);
    seq.create_port(&dinfo)?;

    let input_port = dinfo.get_port();

    Ok((seq, input_port))
}

fn note_name(note: u8) -> String {
    let note_name = match note % 12 {
        0 => "C",
        1 => "C#",
        2 => "D",
        3 => "D#",
        4 => "E",
        5 => "F",
        6 => "F#",
        7 => "G",
        8 => "G#",
        9 => "A",
        10 => "A#",
        11 => "B",
        _ => "??"
    };
    format!("{}{}", note_name, note / 12)
}

impl<'a> MidiMonitor<'a> {
    fn get_origin(&mut self, ev: &seq::Event) -> Result<String, Box<error::Error>> {
        let source = ev.get_source();
        self.get_port_name(source)
    }
    fn get_port_name(&mut self, source: seq::Addr) -> Result<String, Box<error::Error>> {
        match self.port_names.get(&source) {
            Some(name) => {
                return Ok(name.to_string())
            }
            None => {
            }
        }

        let client_info = match self.seq.get_any_client_info(source.client) {
            Ok(info) => info,
            _ => {
                return Ok(format!("{}:{}", source.client, source.port));
            }
        };
        // Not in cache, calculate
        let origin = format!("{}:{}",
            client_info.get_name()?,
            self.seq.get_any_port_info(source)?.get_name()?,
        );
        self.port_names.insert(source, origin);
        let origin = self.port_names.get(&source).ok_or("WTF. I just inserted you.")?;
        Ok(origin.to_string())
    }
    fn remove_port_name(&mut self, source: seq::Addr) {
        self.port_names.remove(&source);
    }
    fn autoconnect_all(&mut self) -> Result<(), Box<error::Error>> {
        let seq = self.seq;
        for from_info in seq::ClientIter::new(&seq){
            for from_port in seq::PortIter::new(&seq, from_info.get_client()){
                if from_port.get_capability().contains(seq::SUBS_READ) && !from_port.get_capability().contains(seq::NO_EXPORT){
                    let sender = seq::Addr{ client: from_port.get_client(), port: from_port.get_port() };
                    self.connect_from(sender)?;
                }
            }
        }

        Ok(())
    }

    fn connect_from(&mut self, sender: seq::Addr) -> Result<(), Box<error::Error>> {
        let subs = seq::PortSubscribe::empty()?;
        subs.set_sender(sender);
        subs.set_dest(seq::Addr{ client: self.seq.client_id()?, port: self.port });
        self.seq.subscribe_port(&subs)?;
        Ok(())
    }
}

fn print_midi_ev(midi_monitor: &mut MidiMonitor, ev: &seq::Event) -> Result<(), Box<error::Error>>{
    let elapsed = midi_monitor.start_time.elapsed();
    let elapsed: f64 = elapsed.as_secs() as f64 + elapsed.subsec_millis() as f64 / 1000.0;
    let mut event;
    let mut extra_data: String = "".to_string();
    let origin = midi_monitor.get_origin(&ev)?;

    match ev.get_type() {
        seq::EventType::Noteon => {
            let data: seq::EvNote = ev.get_data().ok_or("Error resolving event data")?;
            event = if data.velocity > 0 {
                "Note ON ".green()
            } else {
                "Note ON ".red()
            };
            extra_data = format!(
                "Channel {:2} | {:<3} ({}) | {}",
                data.channel.to_string().white().dimmed(),
                note_name(data.note),
                data.note,
                data.velocity
            );
        },
        seq::EventType::Noteoff => {
            event = "Note OFF".red();
            let data: seq::EvNote = ev.get_data().ok_or("Error resolving event data")?;
            extra_data = format!(
                "Channel {:2} | {:<3} ({}) | {}",
                data.channel.to_string().white().dimmed(),
                note_name(data.note),
                data.note,
                data.velocity
            );
        },
        seq::EventType::Controller => {
            let data: seq::EvCtrl = ev.get_data().ok_or("Error resolving event data")?;
            event = "Controller Change".blue();
            extra_data = format!(
                "Channel {:2} | CC {:3} | {:3} | {} ",
                data.channel,
                data.param,
                data.value,
                CC_MAP.get(&data.param).unwrap_or(&"Unknown".to_string()),
            );
        },
        seq::EventType::Pitchbend => {
            let data: seq::EvCtrl = ev.get_data().ok_or("Error resolving event data")?;
            event = "Pitch Bend".purple();
            extra_data = format!(
                "Channel {:2} | {} ",
                data.channel,
                data.value,
            );
        },
        seq::EventType::Pgmchange => {
            let data: seq::EvCtrl = ev.get_data().ok_or("Error resolving event data")?;
            event = "Program Change".purple();
            extra_data = format!(
                "{}",
                data.value,
            );
        },
        seq::EventType::Chanpress => {
            let data: seq::EvCtrl = ev.get_data().ok_or("Error resolving event data")?;
            event = "Channel Pressure".purple();
            extra_data = format!(
                "Channel {:2} | {}",
                data.channel,
                data.value,
            );
        }
        seq::EventType::Clock => {
            midi_monitor.clock_pos += 1;
            midi_monitor.average_sec_per_clock =
                ((elapsed - midi_monitor.last_clock) * BPM_DAMPING) +
                midi_monitor.average_sec_per_clock as f64 * (1.0 - BPM_DAMPING);
            midi_monitor.last_clock = elapsed;

            // I hope RUST simplifies this.. as I prefer clean code.
            let cs = 1.0 / midi_monitor.average_sec_per_clock;
            let bs = cs / 24.0; // 24 clocks per beat -> beats per second
            let bpm = bs * 60.0;

            // Show only once per beat
            if midi_monitor.clock_pos % 24 == 0 {
                print!(
                    "{:10.3} | {:20} | {:>17} | {:>3.1} BPM | Clock Position {}               \r",
                    elapsed, origin, "Clock".purple(), bpm, midi_monitor.clock_pos
                );
                io::stdout().flush()?;
            }
            return Ok(());
        }
        seq::EventType::Start => {
            event = "Start".purple();
            midi_monitor.clock_pos = 0;
        }
        seq::EventType::Stop => {
            event = "Stop".purple();
        }
        seq::EventType::Continue => {
            event = "Continue".purple();
        }
        seq::EventType::ClientStart => {
            event = "ClientStart".green();
            let addr: seq::Addr = ev.get_data().ok_or("Expected address")?;
            extra_data = format!("{}", midi_monitor.get_port_name(addr)?);
        }
        seq::EventType::PortStart => {
            event = "PortStart".green();
            let addr: seq::Addr = ev.get_data().ok_or("Expected address")?;
            if midi_monitor.autoconnect {
                midi_monitor.connect_from(addr)?;
            }
            extra_data = format!("{}", midi_monitor.get_port_name(addr)?);
        }
        seq::EventType::ClientExit => {
            event = "ClientExit".red();
            let addr: seq::Addr = ev.get_data().ok_or("Expected address")?;
            extra_data = format!("{}", midi_monitor.get_port_name(addr)?);
        }
        seq::EventType::PortExit => {
            event = "PortExit".red();
            let addr: seq::Addr = ev.get_data().ok_or("Expected address")?;
            extra_data = format!("{}", midi_monitor.get_port_name(addr)?);
            midi_monitor.remove_port_name(addr);
        }
        seq::EventType::PortSubscribed => {
            event = "PortSubscribed".green();
            let conn: seq::Connect = ev.get_data().ok_or("Expected connection")?;
            extra_data = format!(
                "{:20} | {:20}",
                midi_monitor.get_port_name(conn.sender)?,
                midi_monitor.get_port_name(conn.dest)?
            );
        }
        seq::EventType::PortUnsubscribed => {
            event = "PortUnsubscribed".red();
            let conn: seq::Connect = ev.get_data().ok_or("Expected connection")?;
            extra_data = format!(
                "{:20} | {:20}",
                midi_monitor.get_port_name(conn.sender)?,
                midi_monitor.get_port_name(conn.dest)?
            );
        }
        _ => {
            event = format!("{:?}", ev).cyan();
        }
    }
    println!(
        "{:10.3} | {:20} | {:>17} | {}                                          ",
        elapsed,
        origin,
        event,
        extra_data
    );

    Ok(())
}


fn main() -> Result<(), Box<error::Error>> {
    println!("Terminal MIDI Monitor. (C) 2019 Coralbits SL. Licensed under GPL v3.");
    let matches = App::new("Terminal MIDI Monitor")
        .version("0.1.0")
        .author("David Moreno <dmoreno@coralbits.com>")
        .about("Terminal monitor for Alsa Seq MIDI events.")
        .arg(
            Arg::with_name("autoconnect")
                .short("a")
                .long("autoconnect")
                .help("Autoconnects all outputs to the monitor. Also new clients are automatically connected.")
            )
        .get_matches();
    let autoconnect = matches.occurrences_of("autoconnect") > 0;

    let (seq, port) = setup_alsaseq()?;
    let mut input = seq.input();

    println!("Waiting for connections.");

    use alsa::PollDescriptors;
    let seqp = (&seq, Some(alsa::Direction::Capture));
    let mut fds = Vec::<libc::pollfd>::new();
    fds.append(&mut seqp.get()?);

    let mut midi_monitor = MidiMonitor{
        start_time: Instant::now(),
        seq: &seq,
        average_sec_per_clock: (60.0 / 120.0) / 24.0,
        last_clock: 0.0,
        clock_pos: 0,
        autoconnect: autoconnect,
        port: port,
        port_names: HashMap::new()
    };

    if autoconnect {
        println!("{}", "Autoconnect ON".yellow());
        midi_monitor.autoconnect_all()?;
    }


    loop {
        // FIXME For some events (PortStart,End...) this timeout limits how many to receive per loop.
        alsa::poll::poll(&mut fds, 1000)?;
        while input.event_input_pending(true)? != 0 {
            let ev = input.event_input()?;

            match print_midi_ev(&mut midi_monitor, &ev) {
                Ok(()) => {

                },
                err => {
                    println!("{}", format!("ERROR: {:?}",err).red());
                }
            };
        }
    };

    Ok(())
}
