use memmap2::Mmap;
// use rapidhash::{HashMapExt, RapidHashMap};
use rapidhash::fast::{HashMapExt, RapidHashMap};
use std::fs::File;
use std::sync::Arc;
use std::thread;

const THREAD_N: u8 = 12;

struct Aggregate {
    min: i32,
    max: i32,
    acc: i32,
    count: i32,
}

fn main() -> std::io::Result<()> {
    // let start = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let file = File::open("measurements.txt")?;
    let mmap = Arc::new(unsafe { Mmap::map(&file)? });
    let mut threads_handles: Vec<thread::JoinHandle<RapidHashMap<[u8; 100], Aggregate>>> =
        Vec::new();

    let mut bounds: [u64; THREAD_N as usize + 1] = [0; THREAD_N as usize + 1];
    for thread_id in 1..THREAD_N + 1 {
        let right_bound_naive = mmap.len() as u64 / THREAD_N as u64 * thread_id as u64;
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

    let mut stations: RapidHashMap<[u8; 100], Aggregate> = RapidHashMap::new();
    for handle in threads_handles {
        let joined_stations = handle.join().unwrap();
        stations = merge_stations(stations, joined_stations);
    }

    print_output(&stations);
    Ok(())
}

fn find_next_newline_pos(mut current_pos: u64, mmap: &Mmap) -> u64 {
    if current_pos as usize == mmap.len() {
        return current_pos;
    }
    loop {
        if *mmap.get(current_pos as usize).unwrap() == b'\n' {
            return current_pos + 1;
        }
        current_pos += 1;
    }
}

fn process_lines(mmap: &[u8]) -> RapidHashMap<[u8; 100], Aggregate> {
    let mut stations: RapidHashMap<[u8; 100], Aggregate> = RapidHashMap::new();

    let mut station_acc: [u8; 100] = [59; 100];
    let mut station_cnt: u8 = 0;
    let mut station_flg: bool = false;

    let mut float_acc: i32 = 0;
    let mut float_sign: i8 = 1;

    for i in 0..mmap.len() {
        let b = &mmap[i];

        if *b == b'\n' {
            float_acc *= float_sign as i32;
            process_line(station_acc, float_acc, &mut stations);

            station_acc = [59; 100];
            station_cnt = 0;
            station_flg = false;

            float_acc = 0;
            float_sign = 1;
        } else if *b == b';' {
            station_flg = true;
        } else {
            if !station_flg {
                station_acc[station_cnt as usize] = *b;
                station_cnt += 1;
            } else if *b == b'-' {
                float_sign = -1;
            } else if *b == b'.' {
                continue;
            } else {
                float_acc *= 10;
                float_acc += *b as i32 - 48;
            }
        }
    }

    stations
}

fn process_line(
    station: [u8; 100],
    temperature: i32,
    stations: &mut RapidHashMap<[u8; 100], Aggregate>,
) {
    stations
        .entry(station)
        .and_modify(|a| {
            a.min = i32::min(a.min, temperature);
            a.max = i32::max(a.max, temperature);
            a.count += 1;
            a.acc += temperature;
        })
        .or_insert(Aggregate {
            min: temperature,
            max: temperature,
            acc: temperature,
            count: 1,
        });
}

fn merge_stations(
    mut station1: RapidHashMap<[u8; 100], Aggregate>,
    station2: RapidHashMap<[u8; 100], Aggregate>,
) -> RapidHashMap<[u8; 100], Aggregate> {
    for (k, oa) in station2 {
        station1
            .entry(k)
            .and_modify(|ca| {
                ca.min = i32::min(ca.min, oa.min);
                ca.max = i32::max(ca.max, oa.max);
                ca.acc += oa.acc;
                ca.count += oa.count;
            })
            .or_insert(oa);
    }

    station1
}

fn print_output(stations: &RapidHashMap<[u8; 100], Aggregate>) {
    print!("{{");
    let mut output_vec = Vec::new();
    for (k, v) in stations {
        output_vec.push((k, v));
    }

    let mut is_first = true;
    output_vec.sort_by(|a, b| a.0.cmp(b.0));
    for (k, a) in output_vec {
        if !is_first {
            print!(", ");
        } else {
            is_first = false;
        }

        let mn = a.min as f32 / 10.0;
        let mx = a.max as f32 / 10.0;
        let mean = a.acc as f32 / a.count as f32 / 10.0;

        let mut right_bound = 100;
        for i in 0..k.len() {
            if k[i] == b';' {
                right_bound = i;
                break;
            }
        }
        let station = unsafe { std::str::from_utf8_unchecked(&k[..right_bound]) };
        print!("{station}={mn:.1}/{mean:.1}/{mx:.1}");
    }
    print!("}}");
}
