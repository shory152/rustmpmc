
use crate::{NewData, mpmcq, TEST_QUEUE_SIZE, usizeq};
use std::{thread, time};
use std::sync::{Arc, Mutex, Condvar};
use crate::mpmcq::MyQueue;

pub fn run_queue_test<T:'static+NewData+Send+Sync>(nSender : i32, nReceiver : i32, durationS : i32)
                                                   -> Result<(), &'static str> {
    println!("========= test usize Q ==========");

    let dataQ  = Arc::new(usizeq::UsizeQ::new());

    for i in 0..nReceiver {
        let readQ = dataQ.clone();

        thread::spawn(move || {
            let id = i;
            println!("receiver {} start.", id);

            let begTime = time::Instant::now();
            let mut sum = 0;
            loop {
                let e = readQ.pop();
                sum += 1;
                if sum & 0xffffff == 0 {
                    let elapse = begTime.elapsed();
                    println!("{}: recv {}, {}/ms, {}ns/recv ", id, sum,
                    sum as f64 / elapse.as_millis() as f64, elapse.as_nanos() as f64 / sum as f64);
                }
            }
        });
    }

    for i in 0..nSender {
        let writeQ = dataQ.clone();

        thread::spawn(move || {
            let id = i;
            println!("sender {} start.", id);

            let begTime = time::Instant::now();
            let mut sum = 0i32;
            let e : usize = 1;
            loop {
                writeQ.push(e);
                sum += 1;
                if sum & 0xffffff == 0 {
                    let elapse = begTime.elapsed();
                    println!("{}: send {}, {}/ms, {}ns/send ", id, sum,
                             sum as f64 / elapse.as_millis() as f64, elapse.as_nanos() as f64 / sum as f64);
                }
            }
        });
    }

    let begTime = time::Instant::now();
    thread::sleep(time::Duration::from_secs(durationS as u64));

    Ok(())
}