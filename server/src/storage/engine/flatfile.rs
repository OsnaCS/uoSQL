use super::super::meta::{Table};
use super::super::{Engine, Error};
use std::fs::{OpenOptions, File};
use std::io::{Write, Read, Seek, SeekFrom, Cursor};
use super::super::super::parse::ast;
use super::super::super::parse::ast::CompType;
use super::super::types::SqlType;

//---------------------------------------------------------------
// FlatFile-Engine
//---------------------------------------------------------------
use super::super::data::{Rows};
use std::fs;

pub struct FlatFile<'a> {
    table: Table<'a>,

}

impl<'a> FlatFile<'a> {
    ///
    pub fn new<'b>(table: Table<'b>) -> FlatFile<'b> {
        info!("new flatfile with table: {:?}", table);
        FlatFile { table: table }
    }

    /// return a rows object with the table.dat file as data_src
    pub fn get_reader(&self) -> Result<Rows<File>, Error> {
        let mut file = try!(OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.table.get_table_data_path()));
        info!("opened file {:?}", file);

        Ok(Rows::new(file, &self.table.meta_data.columns))
    }
}

impl<'a> Drop for FlatFile<'a> {
    /// drops the Flatfile
    fn drop(&mut self) {
        info!("drop engine flatfile");
    }
}

impl<'a> Engine for FlatFile<'a> {
    /// creates table for use later
    /// returns with error when it has either no permission or full disk
    fn create_table(&mut self) -> Result<(), Error> {
        let mut _file = try!(OpenOptions::new()
            .write(true)
            .create(true)
            .open(&self.table.get_table_data_path()));

        info!("created file for data: {:?}", _file);

        Ok(())
    }
    /// returns own table
    fn table(&self) -> &Table {
        &self.table
    }

    /// returns all rows which are not deleted
    fn full_scan(&self) -> Result<Rows<Cursor<Vec<u8>>>, Error> {
        let mut reader = try!(self.get_reader());
        let vec: Vec<u8> = Vec::new();
        let cursor = Cursor::new(vec);
        let mut rows = Rows::new(cursor, &self.table.meta_data.columns);
        let mut buf: Vec<u8> = Vec::new();

        while true {
            match reader.next_row(&mut buf) {
                Ok(_) => {
                        rows.add_row(& buf);
                },
                Err(e) => {
                    match e {
                        Error::EndOfFile => break,
                        _ => return Err(e)
                    }
                },
            }
        }
        Ok(rows)
    }

    /// returns an new Rows object which fulfills a constraint
    fn lookup(&self, column_index: usize, value: &[u8], comp: CompType)
    -> Result<Rows<Cursor<Vec<u8>>>, Error>
    {
        let mut reader = try!(self.get_reader());
        reader.lookup(column_index, value, comp)
    }

    /// Inserts a new row with row_data.
    /// Returns the number of rows inserted.
    fn insert_row(&mut self, row_data: &[u8]) -> Result<u64, Error> {
        let mut reader = try!(self.get_reader());
        reader.insert_row(row_data)
    }

    /// delete rows which fulfills a constraint
    /// returns amount of deleted rows
    fn delete(&self, column_index: usize, value: &[u8], comp: CompType)
    -> Result<u64, Error>
    {
        info!("Delete row");
        let mut reader = try!(self.get_reader());
        reader.delete(column_index, value, comp)
    }

    fn modify(&mut self, constraint_column_index: usize,
    constraint_value: &[u8], comp: CompType,
    values: &[(usize, &[u8])] )-> Result<u64, Error>
    {
        info!("modify row");
        let mut reader = try!(self.get_reader());
        reader.modify(constraint_column_index, constraint_value, comp, values)
    }


}
