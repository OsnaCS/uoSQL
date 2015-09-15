use super::{Error};
use std::io::Write;
use std::io::Read;
use super::super::parse::token::Lit;
use super::super::parse::ast::CompType;
use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};
use std::ffi::CString;
use std::str;

/// General enums in SQL
#[derive(Debug, Clone, Copy, RustcDecodable, RustcEncodable, PartialEq)]
pub enum SqlType {
    Int,
    Bool,
    Char(u8),
}


/// Defines the size of Sql data types
/// and returns them
impl SqlType {
    pub fn size(&self) -> u32 {
        match self {
            &SqlType::Int => 4 as u32,
            &SqlType::Bool => 1 as u32,
            &SqlType::Char(len) => (len) as u32,
        }
    }

    /// Decodes the data in buf according to SqlType into a Lit enum.
    pub fn decode_from<R: Read>(&self, mut buf: &mut R) -> Result<Lit, Error> {
        match self {
            &SqlType::Int => {
                let i = try!(buf.read_i32::<BigEndian>());
                Ok(Lit::Int(i as i64))
            },
            &SqlType::Bool => {
                let b = try!(buf.read_u8());
                Ok(Lit::Bool(b))
            },
            &SqlType::Char(_) => {
                let mut s = String::new();
                try!(buf.read_to_string(&mut s));
                Ok(Lit::String(s))
            },
        }
    }


    /// Writes data to buf
    /// Returns the bytes written.
    /// Returns Error::InvalidType if type of Lit does not match expected
    /// type.
    /// Returns byteorder::Error, if data could not be written to buf.
    /// Lit: contains data to write to buf
    /// buf: target of write operation.
    pub fn encode_into<W: Write>(&self, mut buf: &mut W, data: &Lit)
    -> Result<u32, Error>
    {
        match self {
            &SqlType::Int => {
                match data {
                    &Lit::Int(a) => {
                        if a > i32::max_value() as i64 {
                            Err(Error::InvalidType)
                        }
                        else {
                            try!(buf.write_i32::<BigEndian>(a as i32));
                            Ok(self.size())
                        }
                    },
                    _=> {
                        Err(Error::InvalidType)
                    }
                }
            },
            &SqlType::Bool => {
                match data {
                    &Lit::Bool(a) => {
                        try!(buf.write_u8(a as u8));
                        Ok(self.size())
                    }
                    _=> {
                        Err(Error::InvalidType)
                    }
                }
            },
            &SqlType::Char(len) => {
                match data {
                    &Lit::String(ref a) => {
                        let str_as_bytes = Self::to_nul_terminated_bytes(&a, (len + 1) as u32);
                        try!(buf.write_all(&str_as_bytes));
                        Ok(self.size())
                    }
                    _=> {
                        Err(Error::InvalidType)
                    }
                }
            },
        }
    }


    /// Convert s to a vector with l bytes.
    /// If length of s is > l, the returning vector will only contain the first
    /// l bytes.
    /// Otherwise the returned vector will be filled with \0
    /// until it contains l bytes.
    fn to_nul_terminated_bytes(s : &str, l: u32) -> Vec<u8> {
        let mut v = s.to_string().into_bytes();

        v.truncate((l - 1) as usize);

        while v.len() < l as usize {
            v.push(0x00);
        }
        v
    }
    /// compare function that lets you logical compare slices of u8
    /// returns a boolean on success and Error on fail
    /// uses other compare fn for the actual compare
    pub fn cmp(&self, val: &[u8], val2: &[u8], comp: CompType)
    -> Result<bool, Error>
    {
        info!("checking Compare type: {:?}", comp);
        match self {
            &SqlType::Int => {
                match comp {
                    CompType::Equ => {
                        self.equal_for_int_with_value(val, val2)
                    },
                    CompType::NEqu => {
                        self.equal_for_int_with_value(val, val2).map(|x| !x)
                    },
                    CompType::GThan => {
                        self.greater_than_for_int_with_value(val, val2)
                    },
                    CompType::SThan => {
                        self.lesser_than_for_int_with_value(val, val2)
                    },
                    CompType::GEThan => {
                        self.lesser_than_for_int_with_value(val, val2).map(|x| !x)
                    },
                    CompType::SEThan => {
                        self.greater_than_for_int_with_value(val, val2).map(|x| !x)
                    },
                }
            },

            &SqlType::Bool => {
                match comp {
                    CompType::Equ => {
                        self.compare_as_bool(val, val2)
                    },
                    CompType::NEqu => {
                        self.compare_byte_for_equal(val, val2).map(|x| !x)
                    },
                    _ => {
                        Err(Error::NoOperationPossible)
                    }
                }
            },

            &SqlType::Char(_) => {
                match comp {
                    CompType::Equ => {
                        self.compare_byte_for_equal(val, val2)
                    },
                    CompType::NEqu => {
                        self.compare_byte_for_equal(val, val2).map(|x| !x)
                    },
                    CompType::GThan => {
                        self.compare_byte_greater_than(val, val2)
                    },
                    CompType::SThan => {
                        self.compare_byte_lesser_than(val, val2)
                    },
                    CompType::GEThan => {
                        self.compare_byte_lesser_than(val, val2).map(|x| !x)
                    },
                    CompType::SEThan => {
                        self.compare_byte_greater_than(val, val2).map(|x| !x)
                    },
                }
            },
        }
    }
    /// fn compares slices of u8 byte for byte and returns if both values are equal
    /// returns boolean on success and Error when given values do not have the same size
    fn compare_byte_for_equal(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        if val != val2 {
            return Ok(false)
        }
        Ok(true)
    }
    /// fn compares slices of u8 byte for byte and returns
    /// if first given value is greater than the second one
    /// returns boolean on success and Error when given values do not have the same size
    fn compare_byte_greater_than(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        info!("start comparing each byte");
        if val.len() != val2.len() {
            return Err(Error::WrongLength)
        }
        for i in 0 .. val.len() {
            if val[i] > val2[i] {
                return Ok(true)
            }
        }
        Ok(false)
    }

    /// fn compares slices of u8 byte for byte and returns
    /// if first given value is lesser than the second one
    /// returns boolean on success and Error when given values do not have the same size
    fn compare_byte_lesser_than(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        info!("start comparing each byte");
        if val.len() != val2.len() {
            return Err(Error::WrongLength)
        }
        for i in 0 .. val.len() {
            if val[i] < val2[i] {
                return Ok(true)
            }
        }
        Ok(false)
    }

    /// fn compares slices of u8 as booleans and returns
    /// if both both booleans are true
    /// returns boolean on success and Error when given values do not have the same size
    fn compare_as_bool(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        info!("start comparing bool");
        Ok(val == val2)
    }
    /// converts value to i32 and compares if equal (needs 4 bytes)
    /// returns boolean if successful returns Error if not
    fn equal_for_int_with_value(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        info!("start converting to i32");
        let int1: i32 = try!(i32::from_sql(val));
        let int2: i32 = try!(i32::from_sql(val2));
        info!("start comparing i32");
        Ok(int1 == int2)
    }

    /// converts value to i32 and compares if first value is greater (needs 4 bytes)
    /// returns boolean if successful returns Error if not
    fn greater_than_for_int_with_value(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        info!("start converting to i32");
        let int1: i32 = try!(i32::from_sql(val));
        let int2: i32 = try!(i32::from_sql(val2));
        info!("start comparing i32");
        Ok(int1 == int2)
    }

    /// converts value to i32 and compares if first value is lesser (needs 4 bytes)
    /// returns boolean if successful returns Error if not
    fn lesser_than_for_int_with_value(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        info!("start converting to i32");
        let int1: i32 = try!(i32::from_sql(val));
        let int2: i32 = try!(i32::from_sql(val2));
        info!("start comparing i32");
        Ok(int1 < int2)
    }
    /// converts each character into value and uses the average of both val
    /// to determin equal or not
    /// returns boolean if successfull returns Error if not
    fn _equal_for_str_with_value(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        let mut value: u64 = 0;
        let mut value2: u64 = 0;
        info!("starting to calculate value of strings");
        for i in 0 .. val.len() {
            value += val[i] as u64;
        }
        value /= val.len() as u64;
        for i in 0 .. val2.len() {
            value2 += val2[i] as u64;
        }
        value2 /= val2.len() as u64;

        info!("starting to compare the value");
        Ok(value2 == value)
    }
    /// converts each character into value and uses the average of both val
    /// to determin if val is greater than val2
    /// returns boolean if successfull returns Error if not
    fn _greater_than_for_str_with_value(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        let mut value: u64 = 0;
        let mut value2: u64 = 0;
        info!("starting to calculate value of strings");
        for i in 0 .. val.len() {
            value += val[i] as u64;
        }
        value /= val.len() as u64;
        for i in 0 .. val2.len() {
            value2 += val2[i] as u64;
        }
        value2 /= val2.len() as u64;
        info!("starting to compare the value");
        Ok(value > value2)
    }
    /// converts each character into value and uses the average of both val
    /// to determin if val is lesser than val2
    /// returns boolean if successfull returns Error if not
    fn _lesser_than_for_str_with_value(&self, val: &[u8], val2: &[u8])
    -> Result<bool, Error>
    {
        let mut value: u64 = 0;
        let mut value2: u64 = 0;
        info!("starting to calculate value of strings");
        for i in 0 .. val.len() {
            value += val[i] as u64;
        }
        value /= val.len() as u64;
        for i in 0 .. val2.len() {
            value2 += val2[i] as u64;
        }
        value2 /= val2.len() as u64;
        info!("starting to compare the value");
        Ok(value < value2)
    }
}

//---------------------------------------------------------------
// Column
//---------------------------------------------------------------

/// A table column. Has a name, a type, ...
#[derive(Debug,RustcDecodable, RustcEncodable,Clone)]
pub struct Column {
    pub name: String, // name of column
    pub sql_type: SqlType, // name of the data type that is contained in this column
    pub is_primary_key: bool, // defines if column is PK
    pub allow_null: bool, // defines if cloumn allows null
    pub description: String //Displays text describing this column.
}


impl Column {
    /// Creates a new column object
    /// Returns with Column
    pub fn new(
        name: &str,
        sql_type: SqlType,
        allow_null: bool,
        description: &str,
        is_primary_key: bool
        ) -> Column {

        Column {
            name: name.to_string(),
            sql_type: sql_type.clone(),
            allow_null: allow_null,
            description: description.to_string(),
            is_primary_key: is_primary_key
        }
    }

    pub fn get_sql_type(&self) -> &SqlType {
        &self.sql_type
    }

    pub fn get_column_name(&self) -> &str {
        &self.name
    }

    pub fn get_size(&self) -> u32 {
        self.sql_type.size() as u32
    }

}

//---------------------------------------------------------------
// FromSql
//---------------------------------------------------------------

pub trait FromSql {
    fn from_sql(data: &[u8]) -> Result<Self, Error>;
}

impl FromSql for i32 {
    fn from_sql(mut data: &[u8]) -> Result<Self, Error> {
        let i = try!(data.read_i32::<BigEndian>());
        Ok(i)
    }
}

impl FromSql for u16 {
    fn from_sql(mut data: &[u8]) -> Result<Self, Error> {
        let u = try!(data.read_u16::<BigEndian>());
        Ok(u)
    }
}

impl FromSql for u8 {
    fn from_sql(mut data: &[u8]) -> Result<Self, Error> {
        let u = try!(data.read_u8());
        Ok(u)
    }
}

impl FromSql for String {
    fn from_sql(data: &[u8]) -> Result<Self, Error> {
        let cstr = try!(CString::new(data));

        let s = try!(str::from_utf8(cstr.to_bytes())).to_string();
        Ok(s)
    }
}

impl FromSql for bool {
    fn from_sql(mut data: &[u8]) -> Result<Self, Error> {
        Ok(try!(data.read_u8()) != 0)
    }
}
