use std::sync::Arc;

use iced::{Element, Task, widget::button};
use iced_futures::MaybeSend;

#[derive(Debug)]
pub(crate) enum JobState<Ready, Running = ()> {
    /// the has either not yet started or has alreadyfinished.
    Ready(Result<Ready, Arc<eyre::Report>>),
    Running(Running),
}

#[derive(Debug, Clone)]
enum Message<Ready> {
    Trigger,
    Ready(Ready),
}

pub(crate) struct JobView<ActF, Ready, Running = ()> {
    state: JobState<Ready, Running>,
    trigger_title: &'static str,
    act: ActF,
}

impl<ActF, ActO, Ready> JobView<ActF, Ready>
where
    ActF: Fn() -> ActO,
    ActO: Future<Output = Ready> + MaybeSend + 'static,
    Ready: MaybeSend + Clone + 'static,
{
    pub(crate) fn new(state: Ready, trigger_title: &'static str, act: ActF) -> Self {
        Self { state: JobState::Ready(Ok(state)), trigger_title, act }
    }

    fn update(&mut self, msg: Message<Ready>) -> Task<Message<Ready>> {
        match msg {
            Message::Trigger => {
                if let JobState::Running(()) = self.state {
                    return Task::none();
                }

                self.state = JobState::Running(());
                Task::perform((self.act)(), Message::Ready)
            }
            Message::Ready(ready) => {
                self.state = JobState::Ready(Ok(ready));
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message<Ready>> {
        button(self.trigger_title).on_press(Message::Trigger).into()
    }
}
