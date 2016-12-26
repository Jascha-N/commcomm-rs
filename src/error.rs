error_chain! {
    foreign_links {
        TextureCreation(::glium::texture::TextureCreationError);
        ProgramChooserCreation(::glium::program::ProgramChooserCreationError);
        VertexCreation(::glium::vertex::BufferCreationError);
        Draw(::glium::DrawError);
    }

    errors {
        Io(message: String) {
            description(t!("I/O error"))
            display("{}", message)
        }

        ArduinoResponse(command: String, code: ::arduino::ResponseCode) {
            description(t!("Arduino response error"))
            display(t!("Request '{}' failed with error: {}"), command, code)
        }

        ArduinoVerification(reason: Option<String>) {
            description(t!("Arduino verification error"))
            display(t!("Verification failed{}"), reason.as_ref().map_or(String::new(), |reason| format!(": {}", reason)))
        }
    }
}

impl From<::conrod::backend::glium::RendererCreationError> for Error {
    fn from(error: ::conrod::backend::glium::RendererCreationError) -> Self {
        match error {
            ::conrod::backend::glium::RendererCreationError::Texture(error) => error.into(),
            ::conrod::backend::glium::RendererCreationError::Program(error) => error.into()
        }
    }
}

impl From<::conrod::backend::glium::DrawError> for Error {
    fn from(error: ::conrod::backend::glium::DrawError) -> Self {
        match error {
            ::conrod::backend::glium::DrawError::Buffer(error) => error.into(),
            ::conrod::backend::glium::DrawError::Draw(error) => error.into()
        }
    }
}
