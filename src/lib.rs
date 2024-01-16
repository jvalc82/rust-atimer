//! Simple implementation of an asynchronous timer. 
//!
//! A thread is spawned for each instance with the
//! corresponding time amount.

use std::{
    task::{Context,Waker},
    sync::{
        mpsc::{sync_channel, SyncSender, Receiver},
        Arc,
        Mutex
    },
    future::Future,
    pin::Pin,
};

use futures::{
    future::{BoxFuture,FutureExt},
    task::{waker_ref, ArcWake}
};


const MAX_QUEUED_TASKS: usize = 10_000;


pub struct Task {
    // Thread-safe future container for task structure.
    //
    // It should contain a synchronized locking wrapper 
    // and a mpsc SyncSender in order to dispatch the
    // desired task.
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    task_sender: SyncSender<Arc<Task>>
}


pub struct Executor {
    ready_queue: Receiver<Arc<Task>>
}

pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>
}

pub fn new_spawner_executor() -> (Spawner, Executor) {
    //! Task spawner for new timer instance
    //!
    //! Should return a tuple containing Sender and Receiver
    //! instances of the newly created channel.
    //!
    //! Examples
    //! ```
    //! let packet: BoxFuture<_> = Arc::new(BoxFuture::new());
    //! let (transmiter, receiver) = new_spawner_executor();
    //! transmiter.send(packet);
    //! let rpacket: BoxFuture<_> = receiver.recv(); 
    //! ```
    let (task_sender, ready_queue) = sync_channel::<Arc<Task>>(MAX_QUEUED_TASKS);

    ( Spawner{ task_sender }, Executor { ready_queue } )
}

impl Spawner {
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let fut_box = future.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(fut_box)),
            task_sender: self.task_sender.clone()
        });

        self
            .task_sender
            .send(task)
            .expect("Too many queued tasks.");
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        //! Schedules future for execution. 
        //!
        //! Future in Pending state is changed to Ready whenever a task is
        //! polled. This function clones ifself in order to schedule it in
        //! queue.
        let sched_task = arc_self.clone();

        arc_self
            .task_sender
            .send(sched_task)
            .expect("Too many queued tasks.");
    }
}

impl Executor {
    pub fn run(&self) {
        //! Executes future from ready queue. 
        //!
        //! Checks if there is progress to make in spawned future. If still
        //! pending then it just resets locked BoxFuture.
        while let Ok(queued_task) = self.ready_queue.recv() {
            let mut fut_handle = queued_task.future.lock().unwrap();
            
            if let Some(mut fut_box) = fut_handle.take() {
                // Extract a Waker using received future handle from
                // mpsc
                let waker = waker_ref(&queued_task);
                // Retrieve Context from waker instance
                let context = &mut Context::from_waker(&waker);

                
                if fut_box.as_mut().poll(context).is_pending() {
                    *fut_handle = Some(fut_box);
                }
            }
        }
    }
}


pub mod timer {
    use crate::{Waker,Context,Future,Mutex,Arc,Pin};
    use std::{
        time::Duration,
        task::Poll,
        thread
    };

    #[derive(Clone)]
    struct SharedState {
        // Timer internal state information. 
        completed: bool,
        waker: Option<Waker>
    }

    pub struct TimerFuture {
        shared_state: Arc<Mutex<SharedState>>
    }

    impl Future for TimerFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut shared_state = self.shared_state.lock().unwrap();

            if !shared_state.completed {
                shared_state.waker = Some(cx.waker().clone());

                return Poll::Pending;
            }

            Poll::Ready(())
        }
    }

    impl TimerFuture {
        pub fn new(dur: Duration) -> Self {
            //! Returns an instance of asynchronous timer.
            //!
            //! It creates a new system thread in order to schedule task
            //! in ready queue.
            //!
            //! Examples
            //! ```
            //! TimerFuture::new(::std::time::Duration::from_secs(2));
            //! ```    
            let shared_state = Arc::new(Mutex::new(SharedState {
                completed: false,
                waker: None
            }));

            let thrd_state = shared_state.clone();

            thread::spawn(move || {
                thread::sleep(dur);
                let mut thrd_handle = thrd_state.lock().unwrap();
                
                thrd_handle.completed = true;

                // Schedule task for running
                if let Some(waker) = thrd_handle.waker.take() {
                    waker.wake();
                }
            });

            TimerFuture { shared_state }
        }
    }
}
