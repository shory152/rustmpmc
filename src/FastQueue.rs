///
/// my MPMC queue
///

use super::mysync::MyCond;
use std::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use std::mem::swap;

pub struct FQueue<T> {
    closed : AtomicBool,
    buf : Vec<Option<T>>,
    count : AtomicUsize,
    capacity : usize,
    inIdx : usize,
    outIdx : usize,
    condRoom : MyCond,
    condElem : MyCond,
}
//unsafe impl<T: Send> Send for FQueue<T> { }
//unsafe impl<T: Send> Sync for FQueue<T> { }

impl<T> FQueue<T> {
    pub fn new(cap : usize) -> FQueue<T> {
        let mut q = FQueue {
            closed : AtomicBool::new(false),
            buf: Vec::<Option<T>>::with_capacity(cap),
            count: AtomicUsize::new(0),
            capacity: cap,
            inIdx: 0,
            outIdx: 0,
            condRoom: MyCond::new(),
            condElem: MyCond::new(),
        };
        for i in 0..cap {
            q.buf.push(None);
        }
        return q;
    }

    pub fn close(&self) -> bool {
        let hasClosed = self.closed.swap(true, Ordering::SeqCst);
        if hasClosed {
            return false;
        }
        self.notify_all();
        return true;
    }

    fn notify_has_room(&self) {
        let g = self.condRoom.lock();
        self.condRoom.signal();
    }

    fn notify_has_elem(&self) {
        let g = self.condElem.lock();
        self.condElem.signal();
    }

    fn notify_all(&self) {
        let g = self.condRoom.lock();
        self.condRoom.broadcast();
        drop(g);
        let g = self.condElem.lock();
        self.condElem.broadcast();
        drop(g);
    }

    pub fn en_queue(&self, e : T) -> bool{
        if self.closed.load(Ordering::SeqCst) {
            //panic!("queue broken");
            return false;
        }

        let mut g = self.condRoom.lock();
        while self.count.load(Ordering::SeqCst) == self.capacity {
            // no room, queue is full, wait ...
            g = self.condRoom.wait(g);
            if self.closed.load(Ordering::SeqCst) {
                //panic!("queue broken");
                return false;
            }
        }

        // has room, put element into queue
        unsafe {
            let mut nextIn = self.inIdx + 1;
            if nextIn >= self.capacity {
                nextIn = 0;
            }
            *(&self.inIdx as *const usize as *mut usize) = nextIn;

            let addr = &self.buf[self.inIdx] as *const Option<T> as *mut Option<T>;
            swap(&mut Some(e), &mut *addr);
        }


        // inc element count
        let c = self.count.fetch_add(1, Ordering::SeqCst);
        if c + 1 < self.capacity {
            // has more room after I put, notify others to enqueue
            self.condRoom.signal();
        }
        self.condRoom.unlock(g);

        if c == 0 {
            // no element before I putting, maybe someone wait, notify them.
            self.notify_has_elem();
        }

        return true;
    }

    pub fn de_queue(&self) -> Option<T> {
        let mut g = self.condElem.lock();
        while self.count.load(Ordering::SeqCst) == 0 {
            if self.closed.load(Ordering::SeqCst) {
                return None;
            }
            // no element, wait...
            g = self.condElem.wait(g);
        }

        // has element, take it
        let mut e : Option<T> = None;
        unsafe {
            let mut nextOut = self.outIdx + 1;
            if nextOut >= self.capacity {
                nextOut = 0;
            }
            *(&self.outIdx as *const usize as *mut usize) = nextOut;

            swap(&mut e, &mut *(&self.buf[self.outIdx] as *const Option<T> as *mut Option<T>));
        }

        // dec element count
        let c = self.count.fetch_sub(1, Ordering::SeqCst);
        if c > 1 {
            // before I take, there are more elements, notify other to take
            self.condElem.signal();
        }
        self.condElem.unlock(g);

        if c == self.capacity {
            // before I take, there is no room, maybe someone wait for room, notify them.
            self.notify_has_room();
        }

        match e {
            Some(_) => return e,
            None => panic!("not a valid element"),
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::mysync::MyAutoEvent;
    use std::sync::Arc;
    use crate::FastQueue::FQueue;
    use std::thread;
    use std::borrow::BorrowMut;
    use std::cell::RefCell;
    /*
        #[test]
        fn test_fq(){
            let mut evt_ary : Vec<Arc<MyAutoEvent<i64>>> = Vec::new();

            for i in 0..10 {
                evt_ary.push(Arc::new(MyAutoEvent::new()));
            }

            let mut q = Arc::new(FQueue::<i32>::new(100));

            for i in 0..10 {
                let evt = Arc::clone(&evt_ary[i]);
                let qq = Arc::clone(&q);
                thread::spawn(move || {
                    let mut myq = qq;
                    let mut myevt = evt;
                    let mut sum = 0i64;
                    loop {
                        let e = (*myq).de_queue();
                        if let Some(v) = e {
                            sum += v as i64;
                        } else {
                            break;
                        }
                    }
                    myevt.set_event_value(sum);
                });
            }

            let mut sum1 = 0i64;
            for i in 1..10000 {
                q.en_queue(i);
                sum1 += i as i64;
            }

            q.close();
            let mut sum2 = 0i64;
            for i in 0..10 {
                sum2 += evt_ary[i].wait();
            }

            assert_eq!(sum1, sum2);
        }
        */
    #[test]
    fn test_q1(){
        let mut q = FQueue::<i32>::new(10);
        println!("q.buf: {:?}", q.buf);

        q.en_queue(1);
        println!("put 1");
        q.en_queue(2);
        println!("put 2");
        q.en_queue(3);
        println!("put 3");

        let e = q.de_queue();
        println!("take 1");
        assert_eq!(Some(1), e);
        let e = q.de_queue();
        println!("take 2");
        assert_eq!(Some(2), e);
        let e = q.de_queue();
        println!("take 3");
        assert_eq!(Some(3), e);

        q.close();
        let e = q.de_queue();
        println!("take {:?} after close", e);
        assert_eq!(None, e);
    }

    #[test]
    fn test_q2() {
        let q = Arc::new(FQueue::<i32>::new(10));
        q.en_queue(1);
        let e = q.de_queue();
        assert_eq!(Some(1), e);

        let result = Arc::new(FQueue::<i32>::new(10));

        for i in 0..10 {
            let rq = Arc::clone(&q);
            let wq = Arc::clone(&result);
            thread::spawn(move || {
                let id = i;
                let mut sum : i32 = 0;
                for j in 0..100 {
                    let e = rq.de_queue();
                    let v = e.unwrap();
                    sum += v;
                }
                wq.en_queue(sum);
            });
        }

        let mut sum1 = 0;
        for i in 0..1000 {
            let v = i+1;
            sum1 += v;
            q.en_queue(v);
        }

        let mut sum2 = 0;
        for i in 0..10 {
            let v = result.de_queue().unwrap();
            sum2 += v;
        }

        assert_eq!(sum1, sum2);
    }
}