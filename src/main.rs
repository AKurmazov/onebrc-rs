use memchr::memchr_iter;
use memmap2::Mmap;
use std::sync::Arc;
use std::thread;
// use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, fs::File};

const THREAD_N: u8 = 12;

fn main() -> std::io::Result<()> {
    // let start = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let file = File::open("measurements.txt")?;
    let mmap = Arc::new(unsafe { Mmap::map(&file)? });
    let mmap_len = mmap.len();
    let mut threads_handles: Vec<thread::JoinHandle<HashMap<String, (f32, f32, i32, f32)>>> =
        Vec::new();

    let mut bounds: [u64; THREAD_N as usize + 1] = [0; THREAD_N as usize + 1];
    for thread_id in 1..THREAD_N + 1 {
        let right_bound_naive = mmap_len as u64 / THREAD_N as u64 * thread_id as u64;
        bounds[thread_id as usize] = find_next_newline_pos(right_bound_naive, &mmap);
    }

    for thread_id in 1..THREAD_N + 1 {
        let left_bound = bounds[thread_id as usize - 1] as usize;
        let right_bound = bounds[thread_id as usize] as usize;

        let mmap_clone = Arc::clone(&mmap);
        threads_handles.push(thread::spawn(move || {
            process_lines(&mmap_clone[left_bound..right_bound])
        }));
    }

    let mut stations: HashMap<String, (f32, f32, i32, f32)> = HashMap::new();
    for handle in threads_handles {
        let joined_stations = handle.join().unwrap();
        stations = merge_stations(stations, joined_stations);
    }

    // print_output(&stations);

    // let end = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    // let diff = end - start;
    // println!(
    //     "\nElapsed: {} s {} ms",
    //     diff.as_secs(),
    //     diff.subsec_millis()
    // );

    Ok(())
}

fn find_next_newline_pos(mut current_pos: u64, mmap: &Mmap) -> u64 {
    loop {
        if *mmap.get(current_pos as usize).unwrap() == b'\n' {
            return current_pos + 1;
        }
        current_pos += 1;
    }
}

fn process_lines(mmap: &[u8]) -> HashMap<String, (f32, f32, i32, f32)> {
    let mut stations: HashMap<String, (f32, f32, i32, f32)> = HashMap::new();
    let mut line_start = 0;
    for newline_pos in memchr_iter(b'\n', &mmap) {
        let line: &[u8] = &mmap[line_start..newline_pos];
        process_line(line, &mut stations);
        line_start = newline_pos + 1;
    }

    stations
}

fn process_line(line: &[u8], stations: &mut HashMap<String, (f32, f32, i32, f32)>) {
    let s = unsafe { std::str::from_utf8_unchecked(line) };
    let (city, temperature) = split_line(s);
    let city = city.to_owned();
    stations
        .entry(city)
        .and_modify(|(cmin, csum, ccount, cmax)| {
            *cmin = f32::min(*cmin, temperature);
            *csum += temperature;
            *ccount += 1;
            *cmax = f32::max(*cmax, temperature);
        })
        .or_insert((temperature, temperature, 1, temperature));
}

fn split_line(line: &str) -> (&str, f32) {
    let (city, temperature) = line
        .split_once(";")
        .expect("must contain a string and a float");
    (city, temperature.parse::<f32>().expect("must be float"))
}

fn merge_stations(
    mut station1: HashMap<String, (f32, f32, i32, f32)>,
    station2: HashMap<String, (f32, f32, i32, f32)>,
) -> HashMap<String, (f32, f32, i32, f32)> {
    for (k, (omin, osum, ocount, omax)) in station2 {
        station1
            .entry(k)
            .and_modify(|(cmin, csum, ccount, cmax)| {
                *cmin = f32::min(*cmin, omin);
                *csum += osum;
                *ccount += ocount;
                *cmax = f32::max(*cmax, omax);
            })
            .or_insert((omin, osum, ocount, omax));
    }

    station1
}

fn print_output(stations: &HashMap<String, (f32, f32, i32, f32)>) {
    print!("{{");
    let mut output_vec = Vec::new();
    for (k, v) in stations {
        output_vec.push((k, v));
    }

    let mut is_first = true;
    output_vec.sort_by(|a, b| a.0.cmp(b.0));
    for (k, (mn, sm, cnt, mx)) in output_vec {
        if !is_first {
            print!(", ");
        } else {
            is_first = false;
        }

        let mean = *sm / *cnt as f32;
        print!("{k}={mn:.1}/{mean:.1}/{mx:.1}");
    }
    print!("}}");
}
