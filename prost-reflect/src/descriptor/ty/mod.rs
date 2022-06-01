mod build;
#[cfg(test)]
mod tests;

use std::{
    collections::{
        hash_map::{self, HashMap},
        BTreeMap,
    },
    fmt,
    ops::{Range, RangeInclusive},
};

use prost::encoding::WireType;
use prost_types::{
    field_descriptor_proto, DescriptorProto, EnumDescriptorProto, EnumValueDescriptorProto,
    FieldDescriptorProto, FileDescriptorProto, OneofDescriptorProto,
};

use crate::descriptor::{
    debug_fmt_iter, make_full_name, parse_name, parse_namespace, to_index, DescriptorError,
    DescriptorPool, DescriptorPoolRef, FileDescriptor, FileDescriptorRef, MAP_ENTRY_KEY_NUMBER,
    MAP_ENTRY_VALUE_NUMBER,
};

use super::{EnumIndex, EnumValueIndex, ExtensionIndex, FileIndex, MessageIndex, OneofIndex};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct TypeId(field_descriptor_proto::Type, u32);

#[derive(Clone, Default)]
pub(super) struct TypeMap {
    named_types: HashMap<Box<str>, TypeId>,
    messages: Vec<MessageDescriptorInner>,
    enums: Vec<EnumDescriptorInner>,
    extensions: Vec<ExtensionDescriptorInner>,
}

/// A protobuf message definition.
#[derive(Clone, PartialEq, Eq)]
pub struct MessageDescriptor {
    pool: DescriptorPool,
    index: MessageIndex,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MessageDescriptorRef<'a> {
    pool: DescriptorPoolRef<'a>,
    index: MessageIndex,
}

#[derive(Clone)]
struct MessageDescriptorInner {
    full_name: Box<str>,
    file: FileIndex,
    parent: ParentKind,
    is_map_entry: bool,
    fields: BTreeMap<u32, FieldDescriptorInner>,
    field_names: HashMap<Box<str>, u32>,
    field_json_names: HashMap<Box<str>, u32>,
    oneof_decls: Box<[OneofDescriptorInner]>,
    extensions: Vec<ExtensionIndex>,
}

/// A oneof field in a protobuf message.
#[derive(Clone, PartialEq, Eq)]
pub struct OneofDescriptor {
    message: MessageDescriptor,
    index: OneofIndex,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct OneofDescriptorRef<'a> {
    message: MessageDescriptorRef<'a>,
    index: OneofIndex,
}

#[derive(Clone)]
struct OneofDescriptorInner {
    name: Box<str>,
    full_name: Box<str>,
    fields: Vec<u32>,
}

/// A protobuf message definition.
#[derive(Clone, PartialEq, Eq)]
pub struct FieldDescriptor {
    message: MessageDescriptor,
    field: u32,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct FieldDescriptorRef<'a> {
    message: MessageDescriptorRef<'a>,
    field: u32,
}

#[derive(Clone)]
struct FieldDescriptorInner {
    name: Box<str>,
    full_name: Box<str>,
    json_name: Box<str>,
    is_group: bool,
    cardinality: Cardinality,
    is_packed: bool,
    supports_presence: bool,
    default_value: Option<crate::Value>,
    oneof_index: Option<OneofIndex>,
    ty: TypeId,
}

/// A protobuf extension field definition.
#[derive(Clone, PartialEq, Eq)]
pub struct ExtensionDescriptor {
    pool: DescriptorPool,
    index: ExtensionIndex,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ExtensionDescriptorRef<'a> {
    pool: DescriptorPoolRef<'a>,
    index: ExtensionIndex,
}

#[derive(Clone)]
pub struct ExtensionDescriptorInner {
    field: FieldDescriptorInner,
    number: u32,
    file: FileIndex,
    parent: ParentKind,
    extendee: TypeId,
    json_name: Box<str>,
}

/// A protobuf enum type.
#[derive(Clone, PartialEq, Eq)]
pub struct EnumDescriptor {
    pool: DescriptorPool,
    index: EnumIndex,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct EnumDescriptorRef<'a> {
    pool: DescriptorPoolRef<'a>,
    index: EnumIndex,
}

#[derive(Clone)]
struct EnumDescriptorInner {
    full_name: Box<str>,
    file: FileIndex,
    parent: ParentKind,
    value_names: HashMap<Box<str>, EnumValueIndex>,
    values: Vec<EnumValueDescriptorInner>,
    default_value: EnumValueIndex,
}

/// A value in a protobuf enum type.
#[derive(Clone, PartialEq, Eq)]
pub struct EnumValueDescriptor {
    parent: EnumDescriptor,
    index: EnumValueIndex,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct EnumValueDescriptorRef<'a> {
    parent: EnumDescriptorRef<'a>,
    index: EnumValueIndex,
}

#[derive(Clone)]
struct EnumValueDescriptorInner {
    name: Box<str>,
    number: i32,
    full_name: Box<str>,
}

/// The type of a protobuf message field.
#[derive(Clone, PartialEq, Eq)]
pub enum Kind {
    /// The protobuf `double` type.
    Double,
    /// The protobuf `float` type.
    Float,
    /// The protobuf `int32` type.
    Int32,
    /// The protobuf `int64` type.
    Int64,
    /// The protobuf `uint32` type.
    Uint32,
    /// The protobuf `uint64` type.
    Uint64,
    /// The protobuf `sint32` type.
    Sint32,
    /// The protobuf `sint64` type.
    Sint64,
    /// The protobuf `fixed32` type.
    Fixed32,
    /// The protobuf `fixed64` type.
    Fixed64,
    /// The protobuf `sfixed32` type.
    Sfixed32,
    /// The protobuf `sfixed64` type.
    Sfixed64,
    /// The protobuf `bool` type.
    Bool,
    /// The protobuf `string` type.
    String,
    /// The protobuf `bytes` type.
    Bytes,
    /// A protobuf message type.
    Message(MessageDescriptor),
    /// A protobuf enum type.
    Enum(EnumDescriptor),
}

/// Cardinality determines whether a field is optional, required, or repeated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cardinality {
    /// The field appears zero or one times.
    Optional,
    /// The field appears exactly one time. This cardinality is invalid with Proto3.
    Required,
    /// The field appears zero or more times.
    Repeated,
}

#[derive(Copy, Clone, Debug)]
enum ParentKind {
    File,
    Message { index: MessageIndex },
}

impl MessageDescriptor {
    /// Gets a [`MessageDescriptorRef`] referencing this method.
    pub fn as_ref(&self) -> MessageDescriptorRef<'_> {
        MessageDescriptorRef {
            pool: self.pool.as_ref(),
            index: self.index,
        }
    }

    /// Gets a reference to the [`DescriptorPool`] this message is defined in.
    pub fn parent_pool(&self) -> DescriptorPool {
        self.as_ref().parent_pool().to_owned()
    }

    /// Gets the [`FileDescriptor`] this message is defined in.
    pub fn parent_file(&self) -> FileDescriptor {
        self.as_ref().parent_file().to_owned()
    }

    /// Gets the parent message type if this message type is nested inside a another message, or `None` otherwise
    pub fn parent_message(&self) -> Option<MessageDescriptor> {
        self.as_ref()
            .parent_message()
            .map(MessageDescriptorRef::to_owned)
    }

    /// Gets the short name of the message type, e.g. `MyMessage`.
    pub fn name(&self) -> &str {
        self.as_ref().name()
    }

    /// Gets the full name of the message type, e.g. `my.package.MyMessage`.
    pub fn full_name(&self) -> &str {
        self.as_ref().full_name()
    }

    /// Gets the name of the package this message type is defined in, e.g. `my.package`.
    ///
    /// If no package name is set, an empty string is returned.
    pub fn package_name(&self) -> &str {
        self.as_ref().package_name()
    }

    /// Gets a reference to the [`FileDescriptorProto`] in which this message is defined.
    pub fn parent_file_descriptor_proto(&self) -> &FileDescriptorProto {
        self.as_ref().parent_file_descriptor_proto()
    }

    /// Gets a reference to the raw [`DescriptorProto`] wrapped by this [`MessageDescriptor`].
    pub fn descriptor_proto(&self) -> &DescriptorProto {
        self.as_ref().descriptor_proto()
    }

    /// Gets an iterator yielding a [`FieldDescriptor`] for each field defined in this message.
    pub fn fields(&self) -> impl ExactSizeIterator<Item = FieldDescriptor> + '_ {
        self.as_ref().fields().map(FieldDescriptorRef::to_owned)
    }

    /// Gets an iterator yielding a [`OneofDescriptor`] for each oneof field defined in this message.
    pub fn oneofs(&self) -> impl ExactSizeIterator<Item = OneofDescriptor> + '_ {
        self.as_ref().oneofs().map(OneofDescriptorRef::to_owned)
    }

    /// Gets the nested message types defined within this message.
    pub fn child_messages(&self) -> impl ExactSizeIterator<Item = MessageDescriptor> + '_ {
        self.as_ref()
            .child_messages()
            .map(MessageDescriptorRef::to_owned)
    }

    /// Gets the nested enum types defined within this message.
    pub fn child_enums(&self) -> impl ExactSizeIterator<Item = EnumDescriptor> + '_ {
        self.as_ref().child_enums().map(EnumDescriptorRef::to_owned)
    }

    /// Gets the nested extension fields defined within this message.
    ///
    /// Note this only returns extensions defined nested within this message. See
    /// [`MessageDescriptor::extensions`] to get fields defined anywhere that extend this message.
    pub fn child_extensions(&self) -> impl ExactSizeIterator<Item = ExtensionDescriptor> + '_ {
        self.as_ref()
            .child_extensions()
            .map(ExtensionDescriptorRef::to_owned)
    }

    /// Gets an iterator over all extensions to this message defined in the parent [`DescriptorPool`].
    ///
    /// Note this iterates over extension fields defined anywhere which extend this message. See
    /// [`MessageDescriptor::child_extensions`] to just get extensions defined nested within this message.
    pub fn extensions(&self) -> impl ExactSizeIterator<Item = ExtensionDescriptor> + '_ {
        self.as_ref()
            .extensions()
            .map(ExtensionDescriptorRef::to_owned)
    }

    /// Gets a [`FieldDescriptor`] with the given number, or `None` if no such field exists.
    pub fn get_field(&self, number: u32) -> Option<FieldDescriptor> {
        self.as_ref()
            .get_field(number)
            .map(FieldDescriptorRef::to_owned)
    }

    /// Gets a [`FieldDescriptor`] with the given name, or `None` if no such field exists.
    pub fn get_field_by_name(&self, name: &str) -> Option<FieldDescriptor> {
        self.as_ref()
            .get_field_by_name(name)
            .map(FieldDescriptorRef::to_owned)
    }

    /// Gets a [`FieldDescriptor`] with the given JSON name, or `None` if no such field exists.
    pub fn get_field_by_json_name(&self, json_name: &str) -> Option<FieldDescriptor> {
        self.as_ref()
            .get_field_by_json_name(json_name)
            .map(FieldDescriptorRef::to_owned)
    }

    /// Returns `true` if this is an auto-generated message type to
    /// represent the entry type for a map field.
    //
    /// If this method returns `true`, [`fields`][Self::fields] is guaranteed to
    /// yield the following two fields:
    ///
    /// * A "key" field with a field number of 1
    /// * A "value" field with a field number of 2
    ///
    /// See [`map_entry_key_field`][MessageDescriptor::map_entry_key_field] and
    /// [`map_entry_value_field`][MessageDescriptor::map_entry_value_field] for more a convenient way
    /// to get these fields.
    pub fn is_map_entry(&self) -> bool {
        self.as_ref().is_map_entry()
    }

    /// If this is a [map entry](MessageDescriptor::is_map_entry), returns a [`FieldDescriptor`] for the key.
    ///
    /// # Panics
    ///
    /// This method may panic if [`is_map_entry`][MessageDescriptor::is_map_entry] returns `false`.
    pub fn map_entry_key_field(&self) -> FieldDescriptor {
        self.as_ref().map_entry_key_field().to_owned()
    }

    /// If this is a [map entry](MessageDescriptor::is_map_entry), returns a [`FieldDescriptor`] for the value.
    ///
    /// # Panics
    ///
    /// This method may panic if [`is_map_entry`][MessageDescriptor::is_map_entry] returns `false`.
    pub fn map_entry_value_field(&self) -> FieldDescriptor {
        self.as_ref().map_entry_value_field().to_owned()
    }

    /// Gets an iterator over reserved field number ranges in this message.
    pub fn reserved_ranges(&self) -> impl ExactSizeIterator<Item = Range<u32>> + '_ {
        self.as_ref().reserved_ranges()
    }

    /// Gets an iterator over reserved field names in this message.
    pub fn reserved_names(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.as_ref().reserved_names()
    }

    /// Gets an iterator over extension field number ranges in this message.
    pub fn extension_ranges(&self) -> impl ExactSizeIterator<Item = Range<u32>> + '_ {
        self.as_ref().extension_ranges()
    }

    /// Gets an extension to this message by its number, or `None` if no such extension exists.
    pub fn get_extension(&self, number: u32) -> Option<ExtensionDescriptor> {
        self.as_ref()
            .get_extension(number)
            .map(ExtensionDescriptorRef::to_owned)
    }

    /// Gets an extension to this message by its JSON name (e.g. `[my.package.my_extension]`), or `None` if no such extension exists.
    pub fn get_extension_by_json_name(&self, json_name: &str) -> Option<ExtensionDescriptor> {
        self.as_ref()
            .get_extension_by_json_name(json_name)
            .map(ExtensionDescriptorRef::to_owned)
    }
}

impl<'a> MessageDescriptorRef<'a> {
    pub(in crate::descriptor) fn new(pool: DescriptorPoolRef<'a>, ty: TypeId) -> Self {
        debug_assert_eq!(ty.0, field_descriptor_proto::Type::Message);
        MessageDescriptorRef { pool, index: ty.1 }
    }

    pub(in crate::descriptor) fn iter(
        pool: DescriptorPoolRef<'a>,
    ) -> impl ExactSizeIterator<Item = MessageDescriptorRef<'a>> + 'a {
        pool.inner
            .type_map
            .messages()
            .map(move |ty| MessageDescriptorRef::new(pool, ty))
    }

    pub(in crate::descriptor) fn try_get_by_name(
        pool: DescriptorPoolRef<'a>,
        name: &str,
    ) -> Option<MessageDescriptorRef<'a>> {
        let ty = pool.inner.type_map.get_by_name(name)?;
        if !ty.is_message() {
            return None;
        }
        Some(MessageDescriptorRef::new(pool, ty))
    }

    pub fn to_owned(self) -> MessageDescriptor {
        MessageDescriptor {
            pool: self.pool.to_owned(),
            index: self.index,
        }
    }

    pub fn parent_pool(&self) -> DescriptorPoolRef<'a> {
        self.pool
    }

    pub fn parent_file(&self) -> FileDescriptorRef<'a> {
        FileDescriptorRef::new(self.pool, self.inner().file as _)
    }

    pub fn parent_message(&self) -> Option<MessageDescriptorRef<'a>> {
        self.inner()
            .parent
            .as_message()
            .map(|ty| MessageDescriptorRef::new(self.pool, ty))
    }

    pub fn name(&self) -> &'a str {
        parse_name(self.full_name())
    }

    pub fn full_name(&self) -> &'a str {
        &self.inner().full_name
    }

    pub fn package_name(&self) -> &'a str {
        self.parent_file_descriptor_proto().package()
    }

    pub fn parent_file_descriptor_proto(&self) -> &'a FileDescriptorProto {
        self.parent_file().file_descriptor_proto()
    }

    pub fn descriptor_proto(&self) -> &'a DescriptorProto {
        find_message_descriptor_proto(self.parent_pool(), self.inner().file, self.index)
    }

    pub fn fields(&self) -> impl ExactSizeIterator<Item = FieldDescriptorRef<'a>> + 'a {
        let this = *self;
        self.inner()
            .fields
            .keys()
            .map(move |&field| FieldDescriptorRef {
                message: this,
                field,
            })
    }

    pub fn oneofs(&self) -> impl ExactSizeIterator<Item = OneofDescriptorRef<'a>> + 'a {
        let this = *self;
        (0..self.inner().oneof_decls.len())
            .map(move |index| OneofDescriptorRef::new(this, to_index(index)))
    }

    pub fn child_messages(&self) -> impl ExactSizeIterator<Item = MessageDescriptorRef<'a>> + 'a {
        let pool = self.parent_pool();
        let namespace = self.full_name();
        let raw_message = self.descriptor_proto();
        raw_message.nested_type.iter().map(move |raw_nested| {
            pool.get_message_by_name(&make_full_name(namespace, raw_nested.name()))
                .expect("message not found")
        })
    }

    pub fn child_enums(&self) -> impl ExactSizeIterator<Item = EnumDescriptorRef<'a>> + 'a {
        let pool = self.parent_pool();
        let namespace = self.full_name();
        let raw_message = self.descriptor_proto();
        raw_message.enum_type.iter().map(move |raw_enum| {
            pool.get_enum_by_name(&make_full_name(namespace, raw_enum.name()))
                .expect("enum not found")
        })
    }

    pub fn child_extensions(
        &self,
    ) -> impl ExactSizeIterator<Item = ExtensionDescriptorRef<'a>> + 'a {
        let pool = self.parent_pool();
        let namespace = self.full_name();
        let raw_message = self.descriptor_proto();
        raw_message.extension.iter().map(move |raw_extension| {
            let extendee = pool
                .inner
                .type_map
                .resolve_type_name(namespace, raw_extension.extendee())
                .expect("extendee not found");
            MessageDescriptorRef::new(pool, extendee)
                .get_extension(raw_extension.number() as u32)
                .expect("extension not found")
        })
    }

    pub fn extensions(&self) -> impl ExactSizeIterator<Item = ExtensionDescriptorRef<'a>> + 'a {
        let pool = self.parent_pool();
        self.inner()
            .extensions
            .iter()
            .map(move |&index| ExtensionDescriptorRef { pool, index })
    }

    pub fn get_field(&self, number: u32) -> Option<FieldDescriptorRef<'a>> {
        let this = *self;
        if self.inner().fields.contains_key(&number) {
            Some(FieldDescriptorRef {
                message: this,
                field: number,
            })
        } else {
            None
        }
    }

    pub fn get_field_by_name(&self, name: &str) -> Option<FieldDescriptorRef<'a>> {
        let this = *self;
        self.inner()
            .field_names
            .get(name)
            .map(|&number| FieldDescriptorRef {
                message: this,
                field: number,
            })
    }

    pub fn get_field_by_json_name(&self, json_name: &str) -> Option<FieldDescriptorRef<'a>> {
        let this = *self;
        self.inner()
            .field_json_names
            .get(json_name)
            .map(|&number| FieldDescriptorRef {
                message: this,
                field: number,
            })
    }

    pub fn is_map_entry(&self) -> bool {
        self.inner().is_map_entry
    }

    pub fn map_entry_key_field(&self) -> FieldDescriptorRef<'a> {
        debug_assert!(self.is_map_entry());
        self.get_field(MAP_ENTRY_KEY_NUMBER)
            .expect("map entry should have key field")
    }

    pub fn map_entry_value_field(&self) -> FieldDescriptorRef<'a> {
        debug_assert!(self.is_map_entry());
        self.get_field(MAP_ENTRY_VALUE_NUMBER)
            .expect("map entry should have key field")
    }

    pub fn reserved_ranges(&self) -> impl ExactSizeIterator<Item = Range<u32>> + 'a {
        self.descriptor_proto()
            .reserved_range
            .iter()
            .map(|n| (n.start() as u32)..(n.end() as u32))
    }

    pub fn reserved_names(&self) -> impl ExactSizeIterator<Item = &'a str> + 'a {
        self.descriptor_proto()
            .reserved_name
            .iter()
            .map(|n| n.as_ref())
    }

    pub fn extension_ranges(&self) -> impl ExactSizeIterator<Item = Range<u32>> + 'a {
        self.descriptor_proto()
            .extension_range
            .iter()
            .map(|n| (n.start() as u32)..(n.end() as u32))
    }

    pub fn get_extension(&self, number: u32) -> Option<ExtensionDescriptorRef<'a>> {
        self.extensions().find(|ext| ext.number() == number)
    }

    pub fn get_extension_by_json_name(&self, name: &str) -> Option<ExtensionDescriptorRef<'a>> {
        self.extensions().find(|ext| ext.json_name() == name)
    }

    fn inner(&self) -> &'a MessageDescriptorInner {
        self.pool.inner.type_map.get_message(self.index)
    }
}

impl fmt::Debug for MessageDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<'a> fmt::Debug for MessageDescriptorRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageDescriptor")
            .field("name", &self.name())
            .field("full_name", &self.full_name())
            .field("is_map_entry", &self.is_map_entry())
            .field("fields", &debug_fmt_iter(self.fields()))
            .field("oneofs", &debug_fmt_iter(self.oneofs()))
            .finish()
    }
}

impl FieldDescriptor {
    /// Gets a [`FieldDescriptorRef`] referencing this method.
    pub fn as_ref(&self) -> FieldDescriptorRef<'_> {
        FieldDescriptorRef {
            message: self.message.as_ref(),
            field: self.field,
        }
    }

    /// Gets a reference to the [`DescriptorPool`] this field is defined in.
    pub fn parent_pool(&self) -> DescriptorPool {
        self.as_ref().parent_pool().to_owned()
    }

    /// Gets the [`FileDescriptor`] this field is defined in.
    pub fn parent_file(&self) -> FileDescriptor {
        self.as_ref().parent_file().to_owned()
    }

    /// Gets a reference to the [`MessageDescriptor`] this field is defined in.
    pub fn parent_message(&self) -> MessageDescriptor {
        self.as_ref().parent_message().to_owned()
    }

    /// Gets the short name of the message type, e.g. `my_field`.
    pub fn name(&self) -> &str {
        self.as_ref().name()
    }

    /// Gets the full name of the message field, e.g. `my.package.MyMessage.my_field`.
    pub fn full_name(&self) -> &str {
        self.as_ref().full_name()
    }

    /// Gets a reference to the raw [`FieldDescriptorProto`] wrapped by this [`FieldDescriptor`].
    pub fn field_descriptor_proto(&self) -> &FieldDescriptorProto {
        self.as_ref().field_descriptor_proto()
    }

    /// Gets the unique number for this message field.
    pub fn number(&self) -> u32 {
        self.as_ref().number()
    }

    /// Gets the name used for JSON serialization.
    ///
    /// This is usually the camel-cased form of the field name, unless
    /// another value is set in the proto file.
    pub fn json_name(&self) -> &str {
        self.as_ref().json_name()
    }

    /// Whether this field is encoded using the proto2 group encoding.
    pub fn is_group(&self) -> bool {
        self.as_ref().is_group()
    }

    /// Whether this field is a list type.
    ///
    /// Equivalent to checking that the cardinality is `Repeated` and that
    /// [`is_map`][Self::is_map] returns `false`.
    pub fn is_list(&self) -> bool {
        self.as_ref().is_list()
    }

    /// Whether this field is a map type.
    ///
    /// Equivalent to checking that the cardinality is `Repeated` and that
    /// the field type is a message where [`is_map_entry`][MessageDescriptor::is_map_entry]
    /// returns `true`.
    pub fn is_map(&self) -> bool {
        self.as_ref().is_map()
    }

    /// Whether this field is a list encoded using [packed encoding](https://developers.google.com/protocol-buffers/docs/encoding#packed).
    pub fn is_packed(&self) -> bool {
        self.as_ref().is_packed()
    }

    /// The cardinality of this field.
    pub fn cardinality(&self) -> Cardinality {
        self.as_ref().cardinality()
    }

    /// Whether this field supports distinguishing between an unpopulated field and
    /// the default value.
    ///
    /// For proto2 messages this returns `true` for all non-repeated fields.
    /// For proto3 this returns `true` for message fields, and fields contained
    /// in a `oneof`.
    pub fn supports_presence(&self) -> bool {
        self.as_ref().supports_presence()
    }

    /// Gets the [`Kind`] of this field.
    pub fn kind(&self) -> Kind {
        self.as_ref().kind()
    }

    /// Gets a [`OneofDescriptor`] representing the oneof containing this field,
    /// or `None` if this field is not contained in a oneof.
    pub fn containing_oneof(&self) -> Option<OneofDescriptor> {
        self.as_ref()
            .containing_oneof()
            .map(OneofDescriptorRef::to_owned)
    }

    pub(crate) fn default_value(&self) -> Option<&crate::Value> {
        self.as_ref().default_value()
    }

    pub(crate) fn is_packable(&self) -> bool {
        self.as_ref().is_packable()
    }
}

impl<'a> FieldDescriptorRef<'a> {
    pub fn to_owned(self) -> FieldDescriptor {
        FieldDescriptor {
            message: self.message.to_owned(),
            field: self.field,
        }
    }

    pub fn parent_pool(&self) -> DescriptorPoolRef<'a> {
        self.message.parent_pool()
    }

    pub fn parent_file(&self) -> FileDescriptorRef<'a> {
        self.message.parent_file()
    }

    pub fn parent_message(&self) -> MessageDescriptorRef<'a> {
        self.message
    }

    pub fn name(&self) -> &'a str {
        &self.inner().name
    }

    pub fn full_name(&self) -> &'a str {
        &self.inner().full_name
    }

    pub fn field_descriptor_proto(&self) -> &'a FieldDescriptorProto {
        self.parent_message()
            .descriptor_proto()
            .field
            .iter()
            .find(|field| field.number() as u32 == self.field)
            .expect("field not found")
    }

    pub fn number(&self) -> u32 {
        self.field
    }

    pub fn json_name(&self) -> &'a str {
        &self.inner().json_name
    }

    pub fn is_group(&self) -> bool {
        self.inner().is_group
    }

    pub fn is_list(&self) -> bool {
        self.cardinality() == Cardinality::Repeated && !self.is_map()
    }

    pub fn is_map(&self) -> bool {
        self.cardinality() == Cardinality::Repeated
            && match self.kind() {
                Kind::Message(message) => message.is_map_entry(),
                _ => false,
            }
    }

    pub fn is_packed(&self) -> bool {
        self.inner().is_packed
    }

    pub fn cardinality(&self) -> Cardinality {
        self.inner().cardinality
    }

    pub fn supports_presence(&self) -> bool {
        self.inner().supports_presence
    }

    pub fn kind(&self) -> Kind {
        self.inner().ty.to_kind(self.message.pool)
    }

    pub fn containing_oneof(&self) -> Option<OneofDescriptorRef<'a>> {
        self.inner()
            .oneof_index
            .map(|index| OneofDescriptorRef::new(self.message, index))
    }

    pub(crate) fn default_value(&self) -> Option<&'a crate::Value> {
        self.inner().default_value.as_ref()
    }

    pub(crate) fn is_packable(&self) -> bool {
        self.inner().ty.is_packable()
    }

    fn inner(&self) -> &'a FieldDescriptorInner {
        &self.message.inner().fields[&self.field]
    }
}

impl fmt::Debug for FieldDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<'a> fmt::Debug for FieldDescriptorRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldDescriptor")
            .field("name", &self.name())
            .field("full_name", &self.full_name())
            .field("json_name", &self.json_name())
            .field("number", &self.number())
            .field("kind", &self.kind())
            .field("cardinality", &self.cardinality())
            .field(
                "containing_oneof",
                &self.containing_oneof().map(|o| o.name().to_owned()),
            )
            .field("default_value", &self.default_value())
            .field("is_group", &self.is_group())
            .field("is_list", &self.is_list())
            .field("is_map", &self.is_map())
            .field("is_packed", &self.is_packed())
            .field("supports_presence", &self.supports_presence())
            .finish()
    }
}

impl ExtensionDescriptor {
    /// Gets a [`ExtensionDescriptorRef`] referencing this extension.
    pub fn as_ref(&self) -> ExtensionDescriptorRef<'_> {
        ExtensionDescriptorRef {
            pool: self.pool.as_ref(),
            index: self.index,
        }
    }

    /// Gets a reference to the [`DescriptorPool`] this extension field is defined in.
    pub fn parent_pool(&self) -> DescriptorPool {
        self.as_ref().parent_pool().to_owned()
    }

    /// Gets the [`FileDescriptor`] this extension field is defined in.
    pub fn parent_file(&self) -> FileDescriptor {
        self.as_ref().parent_file().to_owned()
    }

    /// Gets the parent message type if this extension is defined within another message, or `None` otherwise.
    ///
    /// Note this just corresponds to where the extension was defined in the proto file. See [`containing_message`][ExtensionDescriptor::containing_message]
    /// for the message this field extends.
    pub fn parent_message(&self) -> Option<MessageDescriptor> {
        self.as_ref()
            .parent_message()
            .map(MessageDescriptorRef::to_owned)
    }

    /// Gets the short name of the extension field type, e.g. `my_extension`.
    pub fn name(&self) -> &str {
        self.as_ref().name()
    }

    /// Gets the full name of the extension field, e.g. `my.package.ParentMessage.my_field`.
    ///
    /// Note this includes the name of the parent message if any, not the message this field extends.
    pub fn full_name(&self) -> &str {
        self.as_ref().full_name()
    }

    /// Gets the name of the package this extension field is defined in, e.g. `my.package`.
    ///
    /// If no package name is set, an empty string is returned.
    pub fn package_name(&self) -> &str {
        self.as_ref().package_name()
    }

    /// Gets a reference to the [`FileDescriptorProto`] in which this extension is defined.
    pub fn parent_file_descriptor_proto(&self) -> &FileDescriptorProto {
        self.as_ref().parent_file_descriptor_proto()
    }

    /// Gets a reference to the raw [`FieldDescriptorProto`] wrapped by this [`ExtensionDescriptor`].
    pub fn field_descriptor_proto(&self) -> &FieldDescriptorProto {
        self.as_ref().field_descriptor_proto()
    }

    /// Gets the number for this extension field.
    pub fn number(&self) -> u32 {
        self.as_ref().number()
    }

    /// Gets the name used for JSON serialization of this extension field, e.g. `[my.package.ParentMessage.my_field]`.
    pub fn json_name(&self) -> &str {
        self.as_ref().json_name()
    }

    /// Whether this field is encoded using the proto2 group encoding.
    pub fn is_group(&self) -> bool {
        self.as_ref().is_group()
    }

    /// Whether this field is a list type.
    ///
    /// Equivalent to checking that the cardinality is `Repeated` and that
    /// [`is_map`][Self::is_map] returns `false`.
    pub fn is_list(&self) -> bool {
        self.as_ref().is_list()
    }

    /// Whether this field is a map type.
    ///
    /// Equivalent to checking that the cardinality is `Repeated` and that
    /// the field type is a message where [`is_map_entry`][MessageDescriptor::is_map_entry]
    /// returns `true`.
    pub fn is_map(&self) -> bool {
        self.as_ref().is_map()
    }

    /// Whether this field is a list encoded using [packed encoding](https://developers.google.com/protocol-buffers/docs/encoding#packed).
    pub fn is_packed(&self) -> bool {
        self.as_ref().is_packed()
    }

    /// The cardinality of this field.
    pub fn cardinality(&self) -> Cardinality {
        self.as_ref().cardinality()
    }

    /// Whether this field supports distinguishing between an unpopulated field and
    /// the default value.
    ///
    /// For proto2 messages this returns `true` for all non-repeated fields.
    /// For proto3 this returns `true` for message fields, and fields contained
    /// in a `oneof`.
    pub fn supports_presence(&self) -> bool {
        self.as_ref().supports_presence()
    }

    /// Gets the [`Kind`] of this field.
    pub fn kind(&self) -> Kind {
        self.as_ref().kind()
    }

    /// Gets the containing message that this field extends.
    pub fn containing_message(&self) -> MessageDescriptor {
        self.as_ref().containing_message().to_owned()
    }

    pub(crate) fn default_value(&self) -> Option<&crate::Value> {
        self.as_ref().default_value()
    }

    pub(crate) fn is_packable(&self) -> bool {
        self.as_ref().is_packable()
    }
}

impl<'a> ExtensionDescriptorRef<'a> {
    pub fn to_owned(self) -> ExtensionDescriptor {
        ExtensionDescriptor {
            pool: self.pool.to_owned(),
            index: self.index,
        }
    }

    pub(in crate::descriptor) fn iter(
        pool: DescriptorPoolRef<'a>,
    ) -> impl ExactSizeIterator<Item = ExtensionDescriptorRef<'a>> + 'a {
        pool.inner
            .type_map
            .extensions()
            .map(move |index| ExtensionDescriptorRef {
                pool,
                index: to_index(index),
            })
    }

    pub fn parent_pool(&self) -> DescriptorPoolRef<'a> {
        self.pool
    }

    pub fn parent_file(&self) -> FileDescriptorRef<'a> {
        FileDescriptorRef::new(self.pool, self.inner().file as _)
    }

    pub fn parent_message(&self) -> Option<MessageDescriptorRef<'a>> {
        self.inner()
            .parent
            .as_message()
            .map(|ty| MessageDescriptorRef::new(self.pool, ty))
    }

    pub fn name(&self) -> &'a str {
        &self.field_inner().name
    }

    pub fn full_name(&self) -> &'a str {
        &self.field_inner().full_name
    }

    pub fn package_name(&self) -> &'a str {
        self.parent_file_descriptor_proto().package()
    }

    pub fn parent_file_descriptor_proto(&self) -> &'a FileDescriptorProto {
        get_file_descriptor_proto(self.pool, self.inner().file)
    }

    pub fn field_descriptor_proto(&self) -> &'a FieldDescriptorProto {
        let name = self.name();
        let inner = self.inner();
        match inner.parent {
            ParentKind::File => get_file_descriptor_proto(self.pool, inner.file)
                .extension
                .iter()
                .find(|extension| extension.name() == name)
                .expect("extension not found"),
            ParentKind::Message {
                index: message_index,
            } => find_message_descriptor_proto(self.pool, inner.file, message_index)
                .extension
                .iter()
                .find(|extension| extension.name() == name)
                .expect("extension not found"),
        }
    }

    pub fn number(&self) -> u32 {
        self.inner().number
    }

    pub fn json_name(&self) -> &'a str {
        &self.inner().json_name
    }

    pub fn is_group(&self) -> bool {
        self.field_inner().is_group
    }

    pub fn is_list(&self) -> bool {
        self.cardinality() == Cardinality::Repeated && !self.is_map()
    }

    pub fn is_map(&self) -> bool {
        self.cardinality() == Cardinality::Repeated
            && match self.kind() {
                Kind::Message(message) => message.is_map_entry(),
                _ => false,
            }
    }

    pub fn is_packed(&self) -> bool {
        self.field_inner().is_packed
    }

    pub fn cardinality(&self) -> Cardinality {
        self.field_inner().cardinality
    }

    pub fn supports_presence(&self) -> bool {
        self.field_inner().supports_presence
    }

    pub fn kind(&self) -> Kind {
        self.field_inner().ty.to_kind(self.pool)
    }

    pub fn containing_message(&self) -> MessageDescriptorRef<'a> {
        MessageDescriptorRef::new(self.pool, self.inner().extendee)
    }

    pub(crate) fn default_value(&self) -> Option<&'a crate::Value> {
        self.field_inner().default_value.as_ref()
    }

    pub(crate) fn is_packable(&self) -> bool {
        self.field_inner().ty.is_packable()
    }

    fn field_inner(&self) -> &'a FieldDescriptorInner {
        &self.inner().field
    }

    fn inner(&self) -> &'a ExtensionDescriptorInner {
        self.pool.inner.type_map.get_extension(self.index)
    }
}

impl fmt::Debug for ExtensionDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<'a> fmt::Debug for ExtensionDescriptorRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionDescriptor")
            .field("name", &self.name())
            .field("full_name", &self.full_name())
            .field("json_name", &self.json_name())
            .field("number", &self.number())
            .field("kind", &self.kind())
            .field("cardinality", &self.cardinality())
            .field(
                "containing_message",
                &self.containing_message().name().to_owned(),
            )
            .field("default_value", &self.default_value())
            .field("is_group", &self.is_group())
            .field("is_list", &self.is_list())
            .field("is_map", &self.is_map())
            .field("is_packed", &self.is_packed())
            .field("supports_presence", &self.supports_presence())
            .finish()
    }
}

impl Kind {
    /// Gets a reference to the [`MessageDescriptor`] if this is a message type,
    /// or `None` otherwise.
    pub fn as_message(&self) -> Option<&MessageDescriptor> {
        match self {
            Kind::Message(desc) => Some(desc),
            _ => None,
        }
    }

    /// Gets a reference to the [`EnumDescriptor`] if this is an enum type,
    /// or `None` otherwise.
    pub fn as_enum(&self) -> Option<&EnumDescriptor> {
        match self {
            Kind::Enum(desc) => Some(desc),
            _ => None,
        }
    }

    pub(crate) fn wire_type(&self) -> WireType {
        match self {
            Kind::Double | Kind::Fixed64 | Kind::Sfixed64 => WireType::SixtyFourBit,
            Kind::Float | Kind::Fixed32 | Kind::Sfixed32 => WireType::ThirtyTwoBit,
            Kind::Enum(_)
            | Kind::Int32
            | Kind::Int64
            | Kind::Uint32
            | Kind::Uint64
            | Kind::Sint32
            | Kind::Sint64
            | Kind::Bool => WireType::Varint,
            Kind::String | Kind::Bytes | Kind::Message(_) => WireType::LengthDelimited,
        }
    }
}

impl fmt::Debug for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Double => write!(f, "double"),
            Self::Float => write!(f, "float"),
            Self::Int32 => write!(f, "int32"),
            Self::Int64 => write!(f, "int64"),
            Self::Uint32 => write!(f, "uint32"),
            Self::Uint64 => write!(f, "uint64"),
            Self::Sint32 => write!(f, "sint32"),
            Self::Sint64 => write!(f, "sint64"),
            Self::Fixed32 => write!(f, "fixed32"),
            Self::Fixed64 => write!(f, "fixed64"),
            Self::Sfixed32 => write!(f, "sfixed32"),
            Self::Sfixed64 => write!(f, "sfixed64"),
            Self::Bool => write!(f, "bool"),
            Self::String => write!(f, "string"),
            Self::Bytes => write!(f, "bytes"),
            Self::Message(m) => write!(f, "{}", m.full_name()),
            Self::Enum(e) => write!(f, "{}", e.full_name()),
        }
    }
}

impl EnumDescriptor {
    /// Gets a [`EnumDescriptorRef`] referencing this enum.
    pub fn as_ref(&self) -> EnumDescriptorRef<'_> {
        EnumDescriptorRef {
            pool: self.pool.as_ref(),
            index: self.index,
        }
    }

    /// Gets a reference to the [`DescriptorPool`] this enum type is defined in.
    pub fn parent_pool(&self) -> DescriptorPool {
        self.as_ref().parent_pool().to_owned()
    }

    /// Gets the [`FileDescriptor`] this enum type is defined in.
    pub fn parent_file(&self) -> FileDescriptor {
        self.as_ref().parent_file().to_owned()
    }

    /// Gets the parent message type if this enum type is nested inside a another message, or `None` otherwise
    pub fn parent_message(&self) -> Option<MessageDescriptor> {
        self.as_ref()
            .parent_message()
            .map(MessageDescriptorRef::to_owned)
    }

    /// Gets the short name of the enum type, e.g. `MyEnum`.
    pub fn name(&self) -> &str {
        self.as_ref().name()
    }

    /// Gets the full name of the enum, e.g. `my.package.MyEnum`.
    pub fn full_name(&self) -> &str {
        self.as_ref().full_name()
    }

    /// Gets the name of the package this enum type is defined in, e.g. `my.package`.
    ///
    /// If no package name is set, an empty string is returned.
    pub fn package_name(&self) -> &str {
        self.as_ref().package_name()
    }

    /// Gets a reference to the [`FileDescriptorProto`] in which this enum is defined.
    pub fn parent_file_descriptor_proto(&self) -> &FileDescriptorProto {
        self.as_ref().parent_file_descriptor_proto()
    }

    /// Gets a reference to the raw [`EnumDescriptorProto`] wrapped by this [`EnumDescriptor`].
    pub fn enum_descriptor_proto(&self) -> &EnumDescriptorProto {
        self.as_ref().enum_descriptor_proto()
    }

    /// Gets the default value for the enum type.
    pub fn default_value(&self) -> EnumValueDescriptor {
        self.as_ref().default_value().to_owned()
    }

    /// Gets a [`EnumValueDescriptor`] for the enum value with the given name, or `None` if no such value exists.
    pub fn get_value_by_name(&self, name: &str) -> Option<EnumValueDescriptor> {
        self.as_ref()
            .get_value_by_name(name)
            .map(EnumValueDescriptorRef::to_owned)
    }

    /// Gets a [`EnumValueDescriptor`] for the enum value with the given number, or `None` if no such value exists.
    ///
    /// If the enum was defined with the `allow_alias` option and has multiple values with the given number, it is
    /// unspecified which one will be returned.
    pub fn get_value(&self, number: i32) -> Option<EnumValueDescriptor> {
        self.as_ref()
            .get_value(number)
            .map(EnumValueDescriptorRef::to_owned)
    }

    /// Gets an iterator yielding a [`EnumValueDescriptor`] for each value in this enum.
    pub fn values(&self) -> impl ExactSizeIterator<Item = EnumValueDescriptor> + '_ {
        self.as_ref().values().map(EnumValueDescriptorRef::to_owned)
    }

    /// Gets an iterator over reserved value number ranges in this enum.
    pub fn reserved_ranges(&self) -> impl ExactSizeIterator<Item = RangeInclusive<i32>> + '_ {
        self.as_ref().reserved_ranges()
    }

    /// Gets an iterator over reserved value names in this enum.
    pub fn reserved_names(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.as_ref().reserved_names()
    }
}

impl<'a> EnumDescriptorRef<'a> {
    pub fn to_owned(self) -> EnumDescriptor {
        EnumDescriptor {
            pool: self.pool.to_owned(),
            index: self.index,
        }
    }

    pub(in crate::descriptor) fn new(pool: DescriptorPoolRef<'a>, ty: TypeId) -> Self {
        debug_assert_eq!(ty.0, field_descriptor_proto::Type::Enum);
        EnumDescriptorRef { pool, index: ty.1 }
    }

    pub(in crate::descriptor) fn iter(
        pool: DescriptorPoolRef<'a>,
    ) -> impl ExactSizeIterator<Item = EnumDescriptorRef<'a>> + 'a {
        pool.inner
            .type_map
            .enums()
            .map(move |ty| EnumDescriptorRef::new(pool, ty))
    }

    pub(in crate::descriptor) fn try_get_by_name(
        pool: DescriptorPoolRef<'a>,
        name: &str,
    ) -> Option<Self> {
        let ty = pool.inner.type_map.get_by_name(name)?;
        if !ty.is_enum() {
            return None;
        }
        Some(EnumDescriptorRef::new(pool, ty))
    }

    pub fn parent_pool(&self) -> DescriptorPoolRef<'a> {
        self.pool
    }

    pub fn parent_file(&self) -> FileDescriptorRef<'a> {
        FileDescriptorRef::new(self.pool, self.inner().file as _)
    }

    pub fn parent_message(&self) -> Option<MessageDescriptorRef<'a>> {
        self.inner()
            .parent
            .as_message()
            .map(|ty| MessageDescriptorRef::new(self.pool, ty))
    }

    pub fn name(&self) -> &'a str {
        parse_name(self.full_name())
    }

    pub fn full_name(&self) -> &'a str {
        &self.inner().full_name
    }

    pub fn package_name(&self) -> &'a str {
        self.parent_file_descriptor_proto().package()
    }

    pub fn parent_file_descriptor_proto(&self) -> &'a FileDescriptorProto {
        get_file_descriptor_proto(self.pool, self.inner().file)
    }

    pub fn enum_descriptor_proto(&self) -> &'a EnumDescriptorProto {
        let name = self.name();
        let inner = self.inner();
        match inner.parent {
            ParentKind::File => get_file_descriptor_proto(self.parent_pool(), inner.file)
                .enum_type
                .iter()
                .find(|extension| extension.name() == name)
                .expect("extension not found"),
            ParentKind::Message {
                index: message_index,
            } => find_message_descriptor_proto(self.parent_pool(), inner.file, message_index)
                .enum_type
                .iter()
                .find(|extension| extension.name() == name)
                .expect("extension not found"),
        }
    }

    pub fn default_value(&self) -> EnumValueDescriptorRef<'a> {
        EnumValueDescriptorRef {
            parent: *self,
            index: self.inner().default_value,
        }
    }

    pub fn get_value_by_name(&self, name: &str) -> Option<EnumValueDescriptorRef<'a>> {
        self.inner()
            .value_names
            .get(name)
            .map(|&index| EnumValueDescriptorRef {
                parent: *self,
                index,
            })
    }

    pub fn get_value(&self, number: i32) -> Option<EnumValueDescriptorRef<'a>> {
        match self
            .inner()
            .values
            .binary_search_by_key(&number, |v| v.number)
        {
            Ok(index) => Some(EnumValueDescriptorRef::new(*self, to_index(index))),
            Err(_) => None,
        }
    }

    pub fn values(&self) -> impl ExactSizeIterator<Item = EnumValueDescriptorRef<'a>> + 'a {
        let this = *self;
        (0..self.inner().values.len())
            .map(move |index| EnumValueDescriptorRef::new(this, to_index(index)))
    }

    pub fn reserved_ranges(&self) -> impl ExactSizeIterator<Item = RangeInclusive<i32>> + 'a {
        self.enum_descriptor_proto()
            .reserved_range
            .iter()
            .map(|n| n.start()..=n.end())
    }

    pub fn reserved_names(&self) -> impl ExactSizeIterator<Item = &'a str> + 'a {
        self.enum_descriptor_proto()
            .reserved_name
            .iter()
            .map(|n| n.as_ref())
    }

    fn inner(&self) -> &'a EnumDescriptorInner {
        self.pool.inner.type_map.get_enum(self.index)
    }
}

impl fmt::Debug for EnumDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<'a> fmt::Debug for EnumDescriptorRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnumDescriptor")
            .field("name", &self.name())
            .field("full_name", &self.full_name())
            .field("default_value", &self.default_value())
            .field("values", &debug_fmt_iter(self.values()))
            .finish()
    }
}

impl EnumValueDescriptor {
    /// Gets a [`EnumValueDescriptorRef`] referencing this enum value.
    pub fn as_ref(&self) -> EnumValueDescriptorRef<'_> {
        EnumValueDescriptorRef {
            parent: self.parent.as_ref(),
            index: self.index,
        }
    }

    /// Gets a reference to the [`DescriptorPool`] this enum value is defined in.
    pub fn parent_pool(&self) -> DescriptorPool {
        self.as_ref().parent_pool().to_owned()
    }

    /// Gets the [`FileDescriptor`] this enum value is defined in.
    pub fn parent_file(&self) -> FileDescriptor {
        self.as_ref().parent_file().to_owned()
    }

    /// Gets a reference to the [`EnumDescriptor`] this enum value is defined in.
    pub fn parent_enum(&self) -> EnumDescriptor {
        self.as_ref().parent_enum().to_owned()
    }

    /// Gets the short name of the enum value, e.g. `MY_VALUE`.
    pub fn name(&self) -> &str {
        self.as_ref().name()
    }

    /// Gets the full name of the enum, e.g. `my.package.MY_VALUE`.
    pub fn full_name(&self) -> &str {
        self.as_ref().full_name()
    }

    /// Gets a reference to the raw [`EnumValueDescriptorProto`] wrapped by this [`EnumValueDescriptor`].
    pub fn enum_value_descriptor_proto(&self) -> &EnumValueDescriptorProto {
        self.as_ref().enum_value_descriptor_proto()
    }

    /// Gets the number representing this enum value.
    pub fn number(&self) -> i32 {
        self.as_ref().number()
    }
}

impl<'a> EnumValueDescriptorRef<'a> {
    fn new(parent: EnumDescriptorRef<'a>, index: EnumValueIndex) -> EnumValueDescriptorRef<'a> {
        EnumValueDescriptorRef { parent, index }
    }

    pub fn to_owned(self) -> EnumValueDescriptor {
        EnumValueDescriptor {
            parent: self.parent.to_owned(),
            index: self.index,
        }
    }

    pub fn parent_pool(&self) -> DescriptorPoolRef<'a> {
        self.parent.parent_pool()
    }

    pub fn parent_file(&self) -> FileDescriptorRef<'a> {
        self.parent.parent_file()
    }

    pub fn parent_enum(&self) -> EnumDescriptorRef<'a> {
        self.parent
    }

    pub fn name(&self) -> &'a str {
        &self.enum_value_ty().name
    }

    pub fn full_name(&self) -> &'a str {
        &self.enum_value_ty().full_name
    }

    pub fn enum_value_descriptor_proto(&self) -> &'a EnumValueDescriptorProto {
        self.parent_enum()
            .enum_descriptor_proto()
            .value
            .iter()
            .find(|value| value.name() == self.name())
            .expect("enum value not found")
    }

    pub fn number(&self) -> i32 {
        self.enum_value_ty().number
    }

    fn enum_value_ty(&self) -> &'a EnumValueDescriptorInner {
        &self.parent.inner().values[self.index as usize]
    }
}

impl fmt::Debug for EnumValueDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<'a> fmt::Debug for EnumValueDescriptorRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnumValueDescriptor")
            .field("name", &self.number())
            .field("full_name", &self.full_name())
            .field("number", &self.number())
            .finish()
    }
}

impl OneofDescriptor {
    /// Gets a [`OneofDescriptorRef`] referencing this oneof.
    pub fn as_ref(&self) -> OneofDescriptorRef<'_> {
        OneofDescriptorRef {
            message: self.message.as_ref(),
            index: self.index,
        }
    }

    /// Gets a reference to the [`DescriptorPool`] this oneof is defined in.
    pub fn parent_pool(&self) -> DescriptorPool {
        self.as_ref().parent_pool().to_owned()
    }

    /// Gets the [`FileDescriptor`] this oneof is defined in.
    pub fn parent_file(&self) -> FileDescriptor {
        self.as_ref().parent_file().to_owned()
    }

    /// Gets a reference to the [`MessageDescriptor`] this oneof is defined in.
    pub fn parent_message(&self) -> MessageDescriptor {
        self.as_ref().parent_message().to_owned()
    }

    /// Gets the short name of the oneof, e.g. `my_oneof`.
    pub fn name(&self) -> &str {
        self.as_ref().name()
    }

    /// Gets the full name of the oneof, e.g. `my.package.MyMessage.my_oneof`.
    pub fn full_name(&self) -> &str {
        self.as_ref().full_name()
    }

    /// Gets a reference to the raw [`OneofDescriptorProto`] wrapped by this [`OneofDescriptor`].
    pub fn oneof_descriptor_proto(&self) -> &OneofDescriptorProto {
        self.as_ref().oneof_descriptor_proto()
    }

    /// Gets an iterator yielding a [`FieldDescriptor`] for each field of the parent message this oneof contains.
    pub fn fields(&self) -> impl ExactSizeIterator<Item = FieldDescriptor> + '_ {
        self.as_ref().fields().map(FieldDescriptorRef::to_owned)
    }
}

impl<'a> OneofDescriptorRef<'a> {
    fn new(message: MessageDescriptorRef<'a>, index: OneofIndex) -> Self {
        OneofDescriptorRef { message, index }
    }

    pub fn to_owned(self) -> OneofDescriptor {
        OneofDescriptor {
            message: self.message.to_owned(),
            index: self.index,
        }
    }

    pub fn parent_pool(&self) -> DescriptorPoolRef<'a> {
        self.message.parent_pool()
    }

    pub fn parent_file(&self) -> FileDescriptorRef<'a> {
        self.message.parent_file()
    }

    pub fn parent_message(&self) -> MessageDescriptorRef<'a> {
        self.message
    }

    pub fn name(&self) -> &'a str {
        &self.oneof_ty().name
    }

    pub fn full_name(&self) -> &'a str {
        &self.oneof_ty().full_name
    }

    pub fn oneof_descriptor_proto(&self) -> &'a OneofDescriptorProto {
        &self.parent_message().descriptor_proto().oneof_decl[self.index as usize]
    }

    pub fn fields(&self) -> impl ExactSizeIterator<Item = FieldDescriptorRef<'a>> + 'a {
        let message = self.message;
        self.oneof_ty()
            .fields
            .iter()
            .map(move |&field| FieldDescriptorRef { message, field })
    }

    fn oneof_ty(&self) -> &'a OneofDescriptorInner {
        &self.message.inner().oneof_decls[self.index as usize]
    }
}

impl fmt::Debug for OneofDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<'a> fmt::Debug for OneofDescriptorRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OneofDescriptor")
            .field("name", &self.name())
            .field("full_name", &self.full_name())
            .field("fields", &debug_fmt_iter(self.fields()))
            .finish()
    }
}

impl TypeMap {
    pub fn shrink_to_fit(&mut self) {
        self.named_types.shrink_to_fit();
        self.messages.shrink_to_fit();
        self.enums.shrink_to_fit();
        self.extensions.shrink_to_fit();
    }

    pub fn try_get_by_name(&self, full_name: &str) -> Result<TypeId, DescriptorError> {
        self.get_by_name(full_name)
            .ok_or_else(|| DescriptorError::type_not_found(full_name))
    }

    pub fn get_by_name(&self, full_name: &str) -> Option<TypeId> {
        let full_name = full_name.strip_prefix('.').unwrap_or(full_name);
        self.named_types.get(full_name).copied()
    }

    pub fn resolve_type_name(
        &self,
        mut namespace: &str,
        type_name: &str,
    ) -> Result<TypeId, DescriptorError> {
        match type_name.strip_prefix('.') {
            Some(full_name) => self.try_get_by_name(full_name),
            None => loop {
                let full_name = make_full_name(namespace, type_name);
                if let Some(ty) = self.get_by_name(&full_name) {
                    break Ok(ty);
                } else if namespace.is_empty() {
                    break Err(DescriptorError::type_not_found(type_name));
                } else {
                    namespace = parse_namespace(namespace);
                }
            },
        }
    }

    fn add_named_type(&mut self, full_name: Box<str>, ty: TypeId) -> Result<(), DescriptorError> {
        let full_name = full_name
            .strip_prefix('.')
            .map(Box::from)
            .unwrap_or(full_name);
        match self.named_types.entry(full_name) {
            hash_map::Entry::Occupied(entry) => {
                Err(DescriptorError::type_already_exists(entry.key()))
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(ty);
                Ok(())
            }
        }
    }

    fn get_message(&self, index: MessageIndex) -> &MessageDescriptorInner {
        &self.messages[index as usize]
    }

    fn get_message_mut(&mut self, ty: TypeId) -> &mut MessageDescriptorInner {
        debug_assert_eq!(ty.0, field_descriptor_proto::Type::Message);
        &mut self.messages[ty.1 as usize]
    }

    fn get_enum(&self, index: EnumIndex) -> &EnumDescriptorInner {
        &self.enums[index as usize]
    }

    fn get_extension(&self, index: ExtensionIndex) -> &ExtensionDescriptorInner {
        &self.extensions[index as usize]
    }

    fn messages(&self) -> impl ExactSizeIterator<Item = TypeId> {
        (0..self.messages.len()).map(|index| TypeId::new_message(to_index(index)))
    }

    fn enums(&self) -> impl ExactSizeIterator<Item = TypeId> {
        (0..self.enums.len()).map(|index| TypeId::new_enum(to_index(index)))
    }

    fn extensions(&self) -> impl ExactSizeIterator<Item = usize> {
        0..self.extensions.len()
    }
}

impl TypeId {
    pub fn new_message(index: MessageIndex) -> Self {
        TypeId(field_descriptor_proto::Type::Message, index)
    }

    pub fn new_enum(index: EnumIndex) -> Self {
        TypeId(field_descriptor_proto::Type::Enum, index)
    }

    pub(crate) fn new_scalar(scalar: field_descriptor_proto::Type) -> Self {
        debug_assert!(
            scalar != field_descriptor_proto::Type::Message
                && scalar != field_descriptor_proto::Type::Enum
                && scalar != field_descriptor_proto::Type::Group
        );
        TypeId(scalar, 0)
    }

    pub fn is_message(&self) -> bool {
        self.0 == field_descriptor_proto::Type::Message
    }

    pub fn is_enum(&self) -> bool {
        self.0 == field_descriptor_proto::Type::Enum
    }

    fn is_packable(&self) -> bool {
        match self.0 {
            field_descriptor_proto::Type::Double
            | field_descriptor_proto::Type::Float
            | field_descriptor_proto::Type::Int64
            | field_descriptor_proto::Type::Uint64
            | field_descriptor_proto::Type::Int32
            | field_descriptor_proto::Type::Fixed64
            | field_descriptor_proto::Type::Fixed32
            | field_descriptor_proto::Type::Bool
            | field_descriptor_proto::Type::Uint32
            | field_descriptor_proto::Type::Enum
            | field_descriptor_proto::Type::Sfixed32
            | field_descriptor_proto::Type::Sfixed64
            | field_descriptor_proto::Type::Sint32
            | field_descriptor_proto::Type::Sint64 => true,
            field_descriptor_proto::Type::String
            | field_descriptor_proto::Type::Bytes
            | field_descriptor_proto::Type::Group
            | field_descriptor_proto::Type::Message => false,
        }
    }

    fn to_kind(self, pool: DescriptorPoolRef) -> Kind {
        match self.0 {
            field_descriptor_proto::Type::Double => Kind::Double,
            field_descriptor_proto::Type::Float => Kind::Float,
            field_descriptor_proto::Type::Int64 => Kind::Int64,
            field_descriptor_proto::Type::Uint64 => Kind::Uint64,
            field_descriptor_proto::Type::Int32 => Kind::Int32,
            field_descriptor_proto::Type::Fixed64 => Kind::Fixed64,
            field_descriptor_proto::Type::Fixed32 => Kind::Fixed32,
            field_descriptor_proto::Type::Bool => Kind::Bool,
            field_descriptor_proto::Type::Uint32 => Kind::Uint32,
            field_descriptor_proto::Type::Sfixed32 => Kind::Sfixed32,
            field_descriptor_proto::Type::Sfixed64 => Kind::Sfixed64,
            field_descriptor_proto::Type::Sint32 => Kind::Sint32,
            field_descriptor_proto::Type::Sint64 => Kind::Sint64,
            field_descriptor_proto::Type::String => Kind::String,
            field_descriptor_proto::Type::Bytes => Kind::Bytes,
            field_descriptor_proto::Type::Enum => {
                Kind::Enum(EnumDescriptorRef::new(pool, self).to_owned())
            }
            field_descriptor_proto::Type::Group | field_descriptor_proto::Type::Message => {
                Kind::Message(MessageDescriptorRef::new(pool, self).to_owned())
            }
        }
    }
}

impl ParentKind {
    fn as_message(&self) -> Option<TypeId> {
        match *self {
            ParentKind::File { .. } => None,
            ParentKind::Message { index } => {
                Some(TypeId(field_descriptor_proto::Type::Message, index))
            }
        }
    }
}

fn get_file_descriptor_proto(pool: DescriptorPoolRef, index: FileIndex) -> &'_ FileDescriptorProto {
    &pool.inner.files[index as usize].raw
}

fn find_message_descriptor_proto(
    pool: DescriptorPoolRef,
    file_index: FileIndex,
    index: MessageIndex,
) -> &'_ DescriptorProto {
    let message = pool.inner.type_map.get_message(index);
    match message.parent {
        ParentKind::File => get_file_descriptor_proto(pool, file_index)
            .message_type
            .iter()
            .find(|ty| ty.name() == parse_name(&message.full_name))
            .expect("message not found"),
        ParentKind::Message {
            index: parent_index,
        } => find_message_descriptor_proto(pool, file_index, parent_index)
            .nested_type
            .iter()
            .find(|ty| ty.name() == parse_name(&message.full_name))
            .expect("message not found"),
    }
}
