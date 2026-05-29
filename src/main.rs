use memchr::memchr_iter;
use memmap2::Mmap;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, fs::File};

fn main() -> std::io::Result<()> {
    let start = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let file = File::open("measurements.txt")?;
    let mmap = Arc::new(unsafe { Mmap::map(&file)? });

    let threads_num: usize = 12;
    let mut threads_handles: Vec<thread::JoinHandle<HashMap<String, (f32, f32, i32, f32)>>> =
        Vec::new();
    let rows_num: usize = 1_000_000_000;

    for thread_id in 1..threads_num + 1 {
        let left_bound = rows_num / threads_num * (thread_id - 1);
        let mut right_bound = rows_num / threads_num * thread_id;

        if thread_id == threads_num {
            right_bound += rows_num % threads_num;
        }

        let mmap_clone = Arc::clone(&mmap);
        threads_handles.push(thread::spawn(move || {
            return process_lines(thread_id, left_bound, right_bound, mmap_clone);
        }));
    }

    let mut stations: HashMap<String, (f32, f32, i32, f32)> = HashMap::new();
    for handle in threads_handles {
        let joined_stations = handle.join().unwrap();
        stations = merge_stations(stations, joined_stations);
    }

    print_output(&stations);

    let end = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let diff = end - start;
    println!(
        "\nElapsed: {} s {} ms",
        diff.as_secs(),
        diff.subsec_millis()
    );

    Ok(())
}

fn process_lines(
    thread_id: usize,
    left_bound: usize,
    right_bound: usize,
    mmap: Arc<Mmap>,
) -> HashMap<String, (f32, f32, i32, f32)> {
    let mut stations: HashMap<String, (f32, f32, i32, f32)> = HashMap::new();

    let mut line_start = 0;
    let mut lines_done = 0;
    for (i, newline_pos) in memchr_iter(b'\n', &mmap).enumerate() {
        if !(i >= left_bound && i <= right_bound) {
            line_start = newline_pos + 1;
            continue;
        }

        let line: &[u8] = &mmap[line_start..newline_pos];
        process_line(line, &mut stations);

        line_start = newline_pos + 1;

        lines_done += 1;
        if lines_done % 1_000_000 == 0 {
            println!("[{}] Done {} lines!", thread_id, lines_done)
        }
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
