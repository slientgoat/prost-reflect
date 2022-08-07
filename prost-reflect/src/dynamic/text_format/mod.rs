use std::{error::Error, fmt};

use crate::{DynamicMessage, MessageDescriptor};

/// An error that may occur while parsing the protobuf text format.
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "text_format")))]
pub struct ParseError {}

impl DynamicMessage {
    /// Parse a [`DynamicMessage`] from the given string encoded using [text format](https://developers.google.com/protocol-buffers/docs/text-format-spec).
    ///
    /// # Examples
    ///
    /// ```
    /// # use prost::Message;
    /// # use prost_reflect::{DynamicMessage, DescriptorPool, Value};
    /// # let pool = DescriptorPool::decode(include_bytes!("../../file_descriptor_set.bin").as_ref()).unwrap();
    /// # let message_descriptor = pool.get_message_by_name("package.MyMessage").unwrap();
    /// let dynamic_message = DynamicMessage::parse_text_format(message_descriptor, "foo: 150").unwrap();
    /// assert_eq!(dynamic_message.get_field_by_name("foo").unwrap().as_ref(), &Value::I32(150));
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "text_format")))]
    pub fn parse_text_format(_desc: MessageDescriptor, _input: &str) -> Result<Self, ParseError> {
        todo!()
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error")
    }
}

impl Error for ParseError {}
