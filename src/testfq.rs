use std::sync::{Arc, Mutex, Condvar};
use crate::{NewData, TEST_QUEUE_SIZE};
use crate::FastQueue::FQueue;
use std::{thread, time};

pub fn run_queue_test<T:'static+NewData+Send+Sync>(nSender : i32, nReceiver : i32, durationS : i32)
    -> Result<(), &'static str> {
    println!("========= test my fast queue ==========");

    let dataQ = Arc::new(FQueue::<T>::new(TEST_QUEUE_SIZE));
    let resultSQ = Arc::new(FQueue::<i32>::new(100));
    let resultRQ = Arc::new(FQueue::<i32>::new(100));

    let condStart = Arc::new((Mutex::new(()), Condvar::new()));

    for i in 0..nReceiver {
        let readQ = Arc::clone(&dataQ);
        let resultQ = Arc::clone(&resultRQ);
        let condS = Arc::clone(&condStart);

        thread::spawn(move || {
            let id = i;
            println!("receiver {} start.", id);
            let (lock, cond) = &*condS;
            let g = lock.lock().unwrap();
            cond.wait(g);

            let begTime = time::Instant::now();
            let mut sum = 0;
            loop {
                let e = readQ.de_queue();
                match e {
                    Some(x) => sum += 1,
                    None => break,
                }
                //if sum & 0xfffff == 0 {
                //    eprint!(".R");
                //}
            }
            let elapse = begTime.elapsed();
            resultQ.en_queue(sum);
            println!("receiver {} end, receive {} times, elapse {} s, {} recv/s, {} ns/recv.",
                     id, sum, elapse.as_secs(), sum as f64 / elapse.as_secs() as f64,
                     elapse.as_nanos() / sum as u128);
        });
    }

    for i in 0..nSender {
        let writeQ = Arc::clone(&dataQ);
        let resultQ = Arc::clone(&resultSQ);
        let condS = Arc::clone(&condStart);

        thread::spawn(move || {
            let id = i;
            println!("sender {} start.", id);
            let (lock, cond) = &*condS;
            let g = lock.lock().unwrap();
            cond.wait(g);

            let begTime = time::Instant::now();
            let mut sum = 0i32;
            loop {
                if !writeQ.en_queue(T::new()) {
                    break;
                }
                sum += 1;
                //if sum & 0xfffff == 0 {
                //    eprint!(".S");
                //}
            }
            let elapse = begTime.elapsed();
            resultQ.en_queue(sum);
            println!("sender {} end, send {} times, elapse {} s, {:.2} send/s, {} ns/send.",
                     id, sum, elapse.as_secs(), sum as f64 / elapse.as_secs() as f64,
                     elapse.as_nanos() / sum as u128);
        });
    }

    thread::sleep(time::Duration::from_secs(2));
    condStart.1.notify_all();

    let begTime = time::Instant::now();
    thread::sleep(time::Duration::from_secs(durationS as u64));
    dataQ.close();

    let mut sumSend = 0;
    for i in 0..nSender {
        let sum = resultSQ.de_queue().unwrap();
        sumSend += sum;
    }
    println!("all sender exit.");

    let mut sumRecv = 0;
    for i in 0..nReceiver {
        let sum = resultRQ.de_queue().unwrap();
        sumRecv += sum;
    }

    let elapse = begTime.elapsed();
    thread::sleep(time::Duration::from_secs(1));

    println!("total send {}, recv {}, cost {} s, data size is {} bytes.",
             sumSend, sumRecv, elapse.as_secs(), std::mem::size_of::<T>());
    println!("  {} send/s, {} recv/s", sumSend as f64 / elapse.as_secs() as f64,
             sumRecv as f64 / elapse.as_secs() as f64,);
    println!("  {} ns/send, {} ns/recv", elapse.as_nanos()/ sumSend as u128,
             elapse.as_nanos()/ sumRecv as u128);

    Ok(())
}