use std::sync::{Arc, Mutex, Condvar};
use crate::NewData;
use crate::FastQueue::FQueue;
use std::{thread, time};

pub fn run_mpscq_test<T:'static+NewData+Send+Sync>(nSender : i32, nReceiver : i32, durationS : i32)
                                                  -> Result<(), &'static str> {
    if nReceiver != 1 {
        return Err("MPSC queue does not support multiple consumers");
    }
    println!("========= test std mpsc queue ==========");

    let boxStopped = Box::new(false);
    let stopped = Box::into_raw(boxStopped) as usize;


    let (writeEnd,readEnd) = std::sync::mpsc::channel::<T>();

    //let dataQ = Arc::new(FQueue::<T>::new(10000));
    let resultSQ = Arc::new(FQueue::<i32>::new(100));
    let resultRQ = Arc::new(FQueue::<i32>::new(100));

    let condStart = Arc::new((Mutex::new(()), Condvar::new()));

    if nReceiver==1 {
        let readQ = readEnd;
        let resultQ = Arc::clone(&resultRQ);
        let condS = Arc::clone(&condStart);

        thread::spawn(move || {
            let id = 1;
            let pStop = stopped as *const bool;
            println!("receiver {} start.", id);
            let (lock, cond) = &*condS;
            let g = lock.lock().unwrap();
            cond.wait(g);

            let begTime = time::Instant::now();
            let mut sum = 0;
            loop {
                //if unsafe{*pStop} {
                //    break;
                //}
                let e = readQ.recv();
                match e {
                    Ok(_) => sum += 1,
                    Err(_) => break,
                }
            }
            let elapse = begTime.elapsed();
            resultQ.en_queue(sum);
            println!("receiver {} end, receive {} times, elapse {} s, {} recv/s, {} ns/recv.",
                     id, sum, elapse.as_secs(), sum as f64 / elapse.as_secs() as f64,
                     elapse.as_nanos() / sum as u128);
        });
    }

    for i in 0..nSender {
        let writeQ = writeEnd.clone();
        let resultQ = Arc::clone(&resultSQ);
        let condS = Arc::clone(&condStart);

        thread::spawn(move || {
            let id = i;
            let pStop = stopped as *mut bool;
            println!("sender {} start.", id);
            let (lock, cond) = &*condS;
            let g = lock.lock().unwrap();
            cond.wait(g);

            let begTime = time::Instant::now();
            let mut sum = 0i32;
            loop {
                if unsafe{*pStop} {
                    break;
                }
                match writeQ.send(T::new()) {
                    Ok(()) => sum += 1,
                    Err(_) => break,
                }
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
    //dataQ.close();
    unsafe{*(stopped as *mut bool) = true;}
    drop(writeEnd);

    let mut sumRecv = 0;
    for i in 0..nReceiver {
        let sum = resultRQ.de_queue().unwrap();
        sumRecv += sum;
    }

    let mut sumSend = 0;
    for i in 0..nSender {
        let sum = resultSQ.de_queue().unwrap();
        sumSend += sum;
    }

    let elapse = begTime.elapsed();
    thread::sleep(time::Duration::from_secs(1));

    println!("total send {}, recv {}, cost {} s, data size is {} bytes.",
             sumSend, sumRecv, elapse.as_secs(), std::mem::size_of::<T>());
    println!("  {} send/s, {} recv/s", sumSend as f64 / elapse.as_secs() as f64,
             sumRecv as f64 / elapse.as_secs() as f64,);
    println!("  {} ns/send, {} ns/recv", elapse.as_nanos()/ sumSend as u128,
             elapse.as_nanos()/ sumRecv as u128);

    unsafe {
        Box::from_raw(stopped as *mut bool);
    }

    Ok(())
}