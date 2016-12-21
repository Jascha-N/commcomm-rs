use error::*;
use super::{cvt, ToWide};
use super::com::{Interface, Pointer};

use kernel32;
use ole32;
use winapi as w;

use std::io;
use std::iter;
use std::mem;
use std::ptr;
use std::slice;
use std::rc::Rc;

#[link(name = "sapi")]
extern {
    static CLSID_SpVoice: w::CLSID;
    static IID_ISpVoice: w::IID;
    static CLSID_SpObjectToken: w::CLSID;
    static IID_ISpObjectToken: w::IID;
    static CLSID_SpObjectTokenCategory: w::CLSID;
    static IID_ISpObjectTokenCategory: w::IID;
}

unsafe impl Interface for w::ISpVoice {
    fn iid() -> &'static w::IID {
        unsafe { &IID_ISpVoice }
    }
}

unsafe impl Interface for w::ISpObjectToken {
    fn iid() -> &'static w::IID {
        unsafe { &IID_ISpObjectToken }
    }
}

unsafe impl Interface for w::ISpObjectTokenCategory {
    fn iid() -> &'static w::IID {
        unsafe { &IID_ISpObjectTokenCategory }
    }
}

pub struct SpeechEngine(());

impl SpeechEngine {
    pub fn new() -> Result<Rc<SpeechEngine>> {
        unsafe {
            cvt(ole32::CoInitializeEx(ptr::null_mut(), w::COINIT_APARTMENTTHREADED))
                .map(|_| Rc::new(SpeechEngine(())))
                .chain_err(|| t!("Could not initialize the COM library"))
        }
    }
}

impl Drop for SpeechEngine {
    fn drop(&mut self) {
        unsafe {
            ole32::CoUninitialize();
        }
    }
}

pub trait SpeechEngineImpl {
    fn voice(&self) -> Result<Voice>;
    fn tokens(&self, category: &str) -> Result<Vec<Token>>;
    fn token_from_id(&self, id: &str) -> Result<Token>;
}

impl SpeechEngineImpl for Rc<SpeechEngine> {
    fn voice(&self) -> Result<Voice> {
        Voice::new(self)
    }

    fn tokens(&self, category: &str) -> Result<Vec<Token>> {
        let raw_tokens = TokenFinder::new(category).find_matching()
            .chain_err(|| t!("Could not enumerate the available tokens"))?;
        let mut tokens = Vec::with_capacity(raw_tokens.len());
        for raw in raw_tokens {
            tokens.push(Token::new(self, raw, None)?);
        }
        Ok(tokens)
    }

    fn token_from_id(&self, id: &str) -> Result<Token> {
        let token = get_token_from_id(id)
            .chain_err(|| format!(t!("Could not find token '{}'"), id))?;
        Token::new(self, token, Some(id))
    }
}

pub struct Voice(Pointer<w::ISpVoice>, Rc<SpeechEngine>);

impl Voice {
    fn new(engine: &Rc<SpeechEngine>) -> Result<Voice> {
        unsafe {
            Pointer::create(&CLSID_SpVoice)
                    .map(|voice| Voice(voice, engine.clone()))
                    .chain_err(|| t!("Could not create a TTS voice"))
        }
    }

    pub fn speak(&mut self, text: &str) -> Result<()> {
        let text = text.to_wide();
        let flags = w::SPF_ASYNC.0 | w::SPF_IS_NOT_XML.0 | w::SPF_PURGEBEFORESPEAK.0;
        unsafe {
            cvt(self.0.Speak(text.as_ptr(), flags, ptr::null_mut()))
                .map(|_| ())
                .chain_err(|| t!("Error while speaking"))
        }
    }

    pub fn set_voice(&mut self, mut token: Token) -> Result<()> {
        unsafe {
            cvt(self.0.SetVoice(&mut *token.token))
                .map(|_| ())
                .chain_err(|| t!("Could not set the TTS voice"))
        }
    }

    pub fn set_volume(&mut self, volume: u8) -> Result<()> {
        unsafe {
            cvt(self.0.SetVolume(volume as w::USHORT))
                .map(|_| ())
                .chain_err(|| t!("Could not set the speech volume"))
        }
    }

    pub fn set_rate(&mut self, rate: i8) -> Result<()> {
        unsafe {
            cvt(self.0.SetRate(rate as w::c_long))
                .map(|_| ())
                .chain_err(|| t!("Could not set the speech rate"))
        }
    }
}

pub struct Token {
    token: Pointer<w::ISpObjectToken>,
    id: String,
    description: String,
    _engine: Rc<SpeechEngine>
}

impl Token {
    fn new(engine: &Rc<SpeechEngine>, mut token: Pointer<w::ISpObjectToken>, id: Option<&str>) -> Result<Token> {
        let description = get_token_description(&mut token)
             .chain_err(|| t!("Could not obtain the token description"))?;

        let id = if let Some(id) = id {
            id.to_string()
        } else {
            get_id_from_token(&mut token).chain_err(|| t!("Could not obtain the token ID"))?
        };

        Ok(Token {
            token: token,
            id: id,
            description: description,
            _engine: engine.clone()
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn token(&self) -> &Pointer<w::ISpObjectToken> {
        &self.token
    }
}



#[derive(Debug)]
pub struct TokenFinder<'a> {
    category: &'a str,
    required: Vec<String>,
    optional: Vec<String>
}

impl<'a> TokenFinder<'a> {
    pub fn new(category: &str) -> TokenFinder {
        TokenFinder {
            category: category,
            required: Vec::new(),
            optional: Vec::new()
        }
    }

    pub fn require<V: ToString>(mut self, attribute: &str, value: V) -> Self {
        self.required.push(format!("{}={}", attribute, value.to_string()));
        self
    }

    pub fn prefer<V: ToString>(mut self, attribute: &str, value: V) -> Self {
        self.optional.push(format!("{}={}", attribute, value.to_string()));
        self
    }

    fn find(&self, vendor_preferred: bool) -> io::Result<Pointer<w::IEnumSpObjectTokens>> {
        let required = self.required.join(";").to_wide();
        let optional = if vendor_preferred {
            self.optional.join(";")
        } else {
            self.optional.iter().map(AsRef::as_ref).chain(iter::once("VendorPreferred")).collect::<Vec<_>>().join(";")
        }.to_wide();
        let mut category = get_category_from_id(&self.category)?;

        unsafe {
            let mut list = mem::uninitialized();
            cvt(category.EnumTokens(required.as_ptr(), optional.as_ptr(), &mut list)).map(|_| {
                Pointer::from_raw(list)
            })
        }
    }

    pub fn find_best(&self) -> io::Result<Option<Pointer<w::ISpObjectToken>>> {
        self.find(true).and_then(|mut list| unsafe {
            let mut token = mem::uninitialized();
            cvt(list.Next(1, &mut token, ptr::null_mut())).map(|result| if result == w::S_OK {
                Some(Pointer::from_raw(token))
            } else {
                None
            })
        })
    }

    pub fn find_matching(&self) -> io::Result<Vec<Pointer<w::ISpObjectToken>>> {
        self.find(false).and_then(|mut list| unsafe {
            let mut count = mem::uninitialized();
            cvt(list.GetCount(&mut count))?;

            let mut buffer = Vec::with_capacity(count as usize);
            buffer.set_len(count as usize);

            cvt(list.Next(count, buffer.as_mut_ptr(), &mut count))?;
            buffer.set_len(count as usize);

            Ok(buffer.into_iter().map(|token| Pointer::from_raw(token)).collect())
        })
    }
}

pub fn get_token_description(token: &mut Pointer<w::ISpObjectToken>) -> io::Result<String> {
    unsafe {
        let mut description = mem::uninitialized();
        cvt(token.GetStringValue(ptr::null(), &mut description)).map(|_| {
            let length = kernel32::lstrlenW(description) as usize;
            let result = String::from_utf16_lossy(slice::from_raw_parts(description, length));
            ole32::CoTaskMemFree(description as *mut _);
            result
        })
    }
}

pub fn get_token_from_id(id: &str) -> io::Result<Pointer<w::ISpObjectToken>> {
    unsafe {
        Pointer::<w::ISpObjectToken>::create(&CLSID_SpObjectToken).and_then(|mut category| {
            cvt(category.SetId(ptr::null(), id.to_wide().as_ptr(), w::FALSE)).map(|_| category)
        })
    }
}

pub fn get_category_from_id(id: &str) -> io::Result<Pointer<w::ISpObjectTokenCategory>> {
    unsafe {
        Pointer::<w::ISpObjectTokenCategory>::create(&CLSID_SpObjectTokenCategory).and_then(|mut category| {
            cvt(category.SetId(id.to_wide().as_ptr(), w::FALSE)).map(|_| category)
        })
    }
}

pub fn get_id_from_token(token: &mut w::ISpObjectToken) -> io::Result<String> {
    unsafe {
        let mut id = mem::uninitialized();
        cvt(token.GetId(&mut id))?;

        let length = kernel32::lstrlenW(id) as usize;
        let result = String::from_utf16_lossy(slice::from_raw_parts(id, length));
        ole32::CoTaskMemFree(id as *mut _);
        Ok(result)
    }
}
