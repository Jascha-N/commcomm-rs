error_chain! {
    errors {
        Io(message: String) {
            description(text!("I/O error"))
            display("{}", message)
        }

        ArduinoResponse(command: String, code: ::arduino::ResponseCode) {
            description(text!("Arduino response error"))
            display(text!("Request '{}' failed with error: {}"), command, code)
        }

        ArduinoVerification(reason: Option<String>) {
            description(text!("Arduino verification error"))
            display(text!("Verification failed{}"), reason.as_ref().map_or(String::new(), |reason| format!(": {}", reason)))
        }
    }
}

pub trait IntoBoxedError {
    fn into_boxed_error(self) -> Box<::std::error::Error + Send>;
}

impl IntoBoxedError for ::conrod::backend::glium::RendererCreationError {
    fn into_boxed_error(self) -> Box<::std::error::Error + Send> {
        match self {
            ::conrod::backend::glium::RendererCreationError::Texture(error) => Box::new(error),
            ::conrod::backend::glium::RendererCreationError::Program(error) => Box::new(error)
        }
    }
}

impl IntoBoxedError for ::conrod::backend::glium::DrawError {
    fn into_boxed_error(self) -> Box<::std::error::Error + Send> {
        match self {
            ::conrod::backend::glium::DrawError::Buffer(error) => Box::new(error),
            ::conrod::backend::glium::DrawError::Draw(error) => Box::new(error)
        }
    }
}