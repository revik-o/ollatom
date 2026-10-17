use super::*;

pub fn execute_sandboxed_command(authorization: SandboxedCommandAuthorization) -> Result<CommandEntity, CommandError> {
    tokio::runtime::Handle::try_current().map_err(|_| CommandError::MissingRuntime)?;
    validate_authorization_identity(&authorization)?;

    let command_text = display_command(&authorization.executable_path, &authorization.arguments);
    let entity_command_text = command_text.clone();
    let terminal_logs = Arc::new(Mutex::new(TerminalLogs::default()));
    let (termination_sender, termination_receiver) = watch::channel(false);
    let (completion_sender, completion_receiver) = oneshot::channel();
    let monitor_terminal_logs = terminal_logs.clone();

    tokio::spawn(async move {
        let result = monitor_command(command_text, authorization, monitor_terminal_logs, termination_receiver).await;
        let _ = completion_sender.send(result);
    });

    Ok(CommandEntity {
        command: entity_command_text,
        terminal_logs,
        termination_sender,
        completion: Some(completion_receiver),
    })
}

impl CommandEntity {
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn terminal_logs(&self) -> TerminalLogs {
        self.terminal_logs.lock().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |terminal_logs| terminal_logs.clone(),
        )
    }

    pub fn terminate(&self) -> Result<(), CommandError> {
        let _ = self.termination_sender.send(true);
        Ok(())
    }
}

impl Drop for CommandEntity {
    fn drop(&mut self) {
        let _ = self.termination_sender.send(true);
    }
}

impl Future for CommandEntity {
    type Output = Result<CommandOutcome, CommandError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(completion_receiver) = self.completion.as_mut() else {
            return Poll::Ready(Err(CommandError::CompletionDisconnected));
        };

        match Pin::new(completion_receiver).poll(context) {
            Poll::Ready(Ok(result)) => {
                self.completion.take();
                Poll::Ready(result)
            }
            Poll::Ready(Err(_)) => {
                self.completion.take();
                Poll::Ready(Err(CommandError::CompletionDisconnected))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
