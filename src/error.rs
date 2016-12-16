error_chain! {
    errors {
        Io(message: String) {
            description("I/O fout")
            display("{}", message)
        }

        ArduinoResponse(command: String, code: ::arduino::ResponseCode) {
            description("Arduino-antwoordfout")
            display("Commando '{}' mislukte met de foutmelding: {}", command, code)
        }

        ArduinoVerification(reason: Option<String>) {
            description("Arduino-verifcatiefout")
            display("Verificatie mislukt{}", reason.as_ref().map_or(String::new(), |reason| format!(": {}", reason)))
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