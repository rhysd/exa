//! The **Details** output view displays each file as a row in a table.
//!
//! It's used in the following situations:
//!
//! - Most commonly, when using the `--long` command-line argument to display the
//!   details of each file, which requires using a table view to hold all the data;
//! - When using the `--tree` argument, which uses the same table view to display
//!   each file on its own line, with the table providing the tree characters;
//! - When using both the `--long` and `--grid` arguments, which constructs a
//!   series of tables to fit all the data on the screen.
//!
//! You will probably recognise it from the `ls --long` command. It looks like
//! this:
//!
//!     .rw-r--r--  9.6k ben 29 Jun 16:16 Cargo.lock
//!     .rw-r--r--   547 ben 23 Jun 10:54 Cargo.toml
//!     .rw-r--r--  1.1k ben 23 Nov  2014 LICENCE
//!     .rw-r--r--  2.5k ben 21 May 14:38 README.md
//!     .rw-r--r--  382k ben  8 Jun 21:00 screenshot.png
//!     drwxr-xr-x     - ben 29 Jun 14:50 src
//!     drwxr-xr-x     - ben 28 Jun 19:53 target
//!
//! The table is constructed by creating a `Table` value, which produces a `Row`
//! value for each file. These rows can contain a vector of `Cell`s, or they can
//! contain depth information for the tree view, or both. These are described
//! below.
//!
//!
//! ## Constructing Detail Views
//!
//! When using the `--long` command-line argument, the details of each file are
//! displayed next to its name.
//!
//! The table holds a vector of all the column types. For each file and column, a
//! `Cell` value containing the ANSI-coloured text and Unicode width of each cell
//! is generated, with the row and column determined by indexing into both arrays.
//!
//! The column types vector does not actually include the filename. This is
//! because the filename is always the rightmost field, and as such, it does not
//! need to have its width queried or be padded with spaces.
//!
//! To illustrate the above:
//!
//!     ┌─────────────────────────────────────────────────────────────────────────┐
//!     │ columns: [ Permissions,  Size,   User,  Date(Modified) ]                │
//!     ├─────────────────────────────────────────────────────────────────────────┤
//!     │   rows:  cells:                                            filename:    │
//!     │   row 1: [ ".rw-r--r--", "9.6k", "ben", "29 Jun 16:16" ]   Cargo.lock   │
//!     │   row 2: [ ".rw-r--r--",  "547", "ben", "23 Jun 10:54" ]   Cargo.toml   │
//!     │   row 3: [ "drwxr-xr-x",    "-", "ben", "29 Jun 14:50" ]   src          │
//!     │   row 4: [ "drwxr-xr-x",    "-", "ben", "28 Jun 19:53" ]   target       │
//!     └─────────────────────────────────────────────────────────────────────────┘
//!
//! Each column in the table needs to be resized to fit its widest argument. This
//! means that we must wait until every row has been added to the table before it
//! can be displayed, in order to make sure that every column is wide enough.
//!
//!
//! ## Constructing Tree Views
//!
//! When using the `--tree` argument, instead of a vector of cells, each row has a
//! `depth` field that indicates how far deep in the tree it is: the top level has
//! depth 0, its children have depth 1, and *their* children have depth 2, and so
//! on.
//!
//! On top of this, it also has a `last` field that specifies whether this is the
//! last row of this particular consecutive set of rows. This doesn't affect the
//! file's information; it's just used to display a different set of Unicode tree
//! characters! The resulting table looks like this:
//!
//!     ┌───────┬───────┬───────────────────────┐
//!     │ Depth │ Last  │ Output                │
//!     ├───────┼───────┼───────────────────────┤
//!     │     0 │       │ documents             │
//!     │     1 │ false │ ├── this_file.txt     │
//!     │     1 │ false │ ├── that_file.txt     │
//!     │     1 │ false │ ├── features          │
//!     │     2 │ false │ │  ├── feature_1.rs   │
//!     │     2 │ false │ │  ├── feature_2.rs   │
//!     │     2 │ true  │ │  └── feature_3.rs   │
//!     │     1 │ true  │ └── pictures          │
//!     │     2 │ false │    ├── garden.jpg     │
//!     │     2 │ false │    ├── flowers.jpg    │
//!     │     2 │ false │    ├── library.png    │
//!     │     2 │ true  │    └── space.tiff     │
//!     └───────┴───────┴───────────────────────┘
//!
//! Creating the table like this means that each file has to be tested to see if
//! it's the last one in the group. This is usually done by putting all the files
//! in a vector beforehand, getting its length, then comparing the index of each
//! file to see if it's the last one. (As some files may not be successfully
//! `stat`ted, we don't know how many files are going to exist in each directory)
//!
//! These rows have a `None` value for their vector of cells, instead of a `Some`
//! vector containing any. It's possible to have *both* a vector of cells and
//! depth and last flags when the user specifies `--tree` *and* `--long`.
//!
//!
//! ## Extended Attributes and Errors
//!
//! Finally, files' extended attributes and any errors that occur while statting
//! them can also be displayed as their children. It looks like this:
//!
//!     .rw-r--r--  0 ben  3 Sep 13:26 forbidden
//!                                    └── <Permission denied (os error 13)>
//!     .rw-r--r--@ 0 ben  3 Sep 13:26 file_with_xattrs
//!                                    ├── another_greeting (len 2)
//!                                    └── greeting (len 5)
//!
//! These lines also have `None` cells, and the error string or attribute details
//! are used in place of the filename.


use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::string::ToString;

use colours::Colours;
use column::{Alignment, Column, Cell};
use dir::Dir;
use feature::xattr::{Attribute, FileAttributes};
use file::fields as f;
use file::File;
use options::{Columns, FileFilter, RecurseOptions, SizeFormat};

use ansi_term::{ANSIString, ANSIStrings, Style};

use datetime::local::{LocalDateTime, DatePiece};
use datetime::format::{DateFormat};
use datetime::zoned::{TimeZone};

use locale;

use number_prefix::{binary_prefix, decimal_prefix, Prefixed, Standalone, PrefixNames};

use users::{OSUsers, Users};
use users::mock::MockUsers;

use super::filename;


/// With the **Details** view, the output gets formatted into columns, with
/// each `Column` object showing some piece of information about the file,
/// such as its size, or its permissions.
///
/// To do this, the results have to be written to a table, instead of
/// displaying each file immediately. Then, the width of each column can be
/// calculated based on the individual results, and the fields are padded
/// during output.
///
/// Almost all the heavy lifting is done in a Table object, which handles the
/// columns for each row.
#[derive(PartialEq, Debug, Copy, Clone, Default)]
pub struct Details {

    /// A Columns object that says which columns should be included in the
    /// output in the general case. Directories themselves can pick which
    /// columns are *added* to this list, such as the Git column.
    pub columns: Option<Columns>,

    /// Whether to recurse through directories with a tree view, and if so,
    /// which options to use. This field is only relevant here if the `tree`
    /// field of the RecurseOptions is `true`.
    pub recurse: Option<RecurseOptions>,

    /// How to sort and filter the files after getting their details.
    pub filter: FileFilter,

    /// Whether to show a header line or not.
    pub header: bool,

    /// Whether to show each file's extended attributes.
    pub xattr: bool,

    /// The colours to use to display information in the table, including the
    /// colour of the tree view symbols.
    pub colours: Colours,
}

impl Details {

    /// Print the details of the given vector of files -- all of which will
    /// have been read from the given directory, if present -- to stdout.
    pub fn view(&self, dir: Option<&Dir>, files: Vec<File>) {

        // First, transform the Columns object into a vector of columns for
        // the current directory.
        let columns_for_dir = match self.columns {
            Some(cols) => cols.for_dir(dir),
            None => Vec::new(),
        };

        // Next, add a header if the user requests it.
        let mut table = Table::with_options(self.colours, columns_for_dir);
        if self.header { table.add_header() }

        // Then add files to the table and print it out.
        self.add_files_to_table(&mut table, files, 0);
        for cell in table.print_table() {
            println!("{}", cell.text);
        }
    }

    /// Adds files to the table, possibly recursively. This is easily
    /// parallelisable, and uses a pool of threads.
    fn add_files_to_table<'dir, U: Users+Send>(&self, mut table: &mut Table<U>, src: Vec<File<'dir>>, depth: usize) {
        use num_cpus;
        use scoped_threadpool::Pool;
        use std::sync::{Arc, Mutex};

        let mut pool = Pool::new(num_cpus::get() as u32);
        let mut file_eggs = Vec::new();

        struct Egg<'_> {
            cells:   Vec<Cell>,
            name:    Cell,
            xattrs:  Vec<Attribute>,
            errors:  Vec<(io::Error, Option<PathBuf>)>,
            dir:     Option<Dir>,
            file:    Arc<File<'_>>,
        }

        pool.scoped(|scoped| {
            let file_eggs = Arc::new(Mutex::new(&mut file_eggs));
            let table = Arc::new(Mutex::new(&mut table));

            for file in src.into_iter() {
                let file: Arc<File> = Arc::new(file);
                let file_eggs = file_eggs.clone();
                let table = table.clone();

                scoped.execute(move || {
                    let mut errors = Vec::new();

                    let mut xattrs = Vec::new();
                    match file.path.attributes() {
                        Ok(xs) => {
                            if self.xattr {
                                for xattr in xs {
                                    xattrs.push(xattr);
                                }
                            }
                        },
                        Err(e) => {
                            if self.xattr {
                                errors.push((e, None));
                            }
                        },
                    };

                    let cells = table.lock().unwrap().cells_for_file(&file, !xattrs.is_empty());

                    let name = Cell {
                        text: filename(&file, &self.colours, true),
                        length: file.file_name_width()
                    };

                    let mut dir = None;

                    if let Some(r) = self.recurse {
                        if file.is_directory() && r.tree && !r.is_too_deep(depth) {
                            if let Ok(d) = file.to_dir(false) {
                                dir = Some(d);
                            }
                        }
                    };

                    let egg = Egg {
                        cells: cells,
                        name: name,
                        xattrs: xattrs,
                        errors: errors,
                        dir: dir,
                        file: file,
                    };

                    file_eggs.lock().unwrap().push(egg);
                });
            }
        });

        file_eggs.sort_by(|a, b| self.filter.compare_files(&*a.file, &*b.file));

        let num_eggs = file_eggs.len();
        for (index, egg) in file_eggs.into_iter().enumerate() {
            let mut files = Vec::new();
            let mut errors = egg.errors;

            let row = Row {
                depth:    depth,
                cells:    Some(egg.cells),
                name:     egg.name,
                last:     index == num_eggs - 1,
            };

            table.rows.push(row);

            if let Some(ref dir) = egg.dir {
                for file_to_add in dir.files() {
                    match file_to_add {
                        Ok(f)          => files.push(f),
                        Err((path, e)) => errors.push((e, Some(path)))
                    }
                }

                self.filter.filter_files(&mut files);

                if !files.is_empty() {
                    for xattr in egg.xattrs {
                        table.add_xattr(xattr, depth + 1, false);
                    }

                    for (error, path) in errors {
                        table.add_error(&error, depth + 1, false, path);
                    }

                    self.add_files_to_table(table, files, depth + 1);
                    continue;
                }
            }

            let count = egg.xattrs.len();
            for (index, xattr) in egg.xattrs.into_iter().enumerate() {
                table.add_xattr(xattr, depth + 1, errors.is_empty() && index == count - 1);
            }

            let count = errors.len();
            for (index, (error, path)) in errors.into_iter().enumerate() {
                table.add_error(&error, depth + 1, index == count - 1, path);
            }
        }
    }
}


struct Row {

    /// Vector of cells to display.
    ///
    /// Most of the rows will be used to display files' metadata, so this will
    /// almost always be `Some`, containing a vector of cells. It will only be
    /// `None` for a row displaying an attribute or error, neither of which
    /// have cells.
    cells: Option<Vec<Cell>>,

    // Did You Know?
    // A Vec<Cell> and an Option<Vec<Cell>> actually have the same byte size!

    /// This file's name, in coloured output. The name is treated separately
    /// from the other cells, as it never requires padding.
    name: Cell,

    /// How many directories deep into the tree structure this is. Directories
    /// on top have depth 0.
    depth: usize,

    /// Whether this is the last entry in the directory. This flag is used
    /// when calculating the tree view.
    last: bool,
}

impl Row {

    /// Gets the Unicode display width of the indexed column, if present. If
    /// not, returns 0.
    fn column_width(&self, index: usize) -> usize {
        match self.cells {
            Some(ref cells) => cells[index].length,
            None => 0,
        }
    }
}


/// A **Table** object gets built up by the view as it lists files and
/// directories.
pub struct Table<U> {
    columns:  Vec<Column>,
    rows:     Vec<Row>,

    time:         locale::Time,
    numeric:      locale::Numeric,
    tz:           TimeZone,
    users:        U,
    colours:      Colours,
    current_year: i64,
}

impl Default for Table<MockUsers> {
    fn default() -> Table<MockUsers> {
        Table {
            columns: Columns::default().for_dir(None),
            rows:    Vec::new(),
            time:    locale::Time::english(),
            numeric: locale::Numeric::english(),
            tz:      TimeZone::localtime().unwrap(),
            users:   MockUsers::with_current_uid(0),
            colours: Colours::default(),
            current_year: 1234,
        }
    }
}

impl Table<OSUsers> {

    /// Create a new, empty Table object, setting the caching fields to their
    /// empty states.
    pub fn with_options(colours: Colours, columns: Vec<Column>) -> Table<OSUsers> {
        Table {
            columns: columns,
            rows:    Vec::new(),

            time:         locale::Time::load_user_locale().unwrap_or_else(|_| locale::Time::english()),
            numeric:      locale::Numeric::load_user_locale().unwrap_or_else(|_| locale::Numeric::english()),
            tz:           TimeZone::localtime().unwrap(),
            users:        OSUsers::empty_cache(),
            colours:      colours,
            current_year: LocalDateTime::now().year(),
        }
    }
}

impl<U> Table<U> where U: Users {

    /// Add a dummy "header" row to the table, which contains the names of all
    /// the columns, underlined. This has dummy data for the cases that aren't
    /// actually used, such as the depth or list of attributes.
    pub fn add_header(&mut self) {
        let row = Row {
            depth:    0,
            cells:    Some(self.columns.iter().map(|c| Cell::paint(self.colours.header, c.header())).collect()),
            name:     Cell::paint(self.colours.header, "Name"),
            last:     false,
        };

        self.rows.push(row);
    }

    fn add_error(&mut self, error: &io::Error, depth: usize, last: bool, path: Option<PathBuf>) {
        let error_message = match path {
            Some(path) => format!("<{}: {}>", path.display(), error),
            None       => format!("<{}>", error),
        };

        let row = Row {
            depth:    depth,
            cells:    None,
            name:     Cell::paint(self.colours.broken_arrow, &error_message),
            last:     last,
        };

        self.rows.push(row);
    }

    fn add_xattr(&mut self, xattr: Attribute, depth: usize, last: bool) {
        let row = Row {
            depth:    depth,
            cells:    None,
            name:     Cell::paint(self.colours.perms.attribute, &format!("{} (len {})", xattr.name, xattr.size)),
            last:     last,
        };

        self.rows.push(row);
    }

    pub fn add_file_with_cells(&mut self, cells: Vec<Cell>, file: &File, depth: usize, last: bool, links: bool) {
        let row = Row {
            depth:    depth,
            cells:    Some(cells),
            name:     Cell { text: filename(file, &self.colours, links), length: file.file_name_width() },
            last:     last,
        };

        self.rows.push(row);
    }

    /// Use the list of columns to find which cells should be produced for
    /// this file, per-column.
    pub fn cells_for_file(&mut self, file: &File, xattrs: bool) -> Vec<Cell> {
        self.columns.clone().iter()
                    .map(|c| self.display(file, c, xattrs))
                    .collect()
    }

    fn display(&mut self, file: &File, column: &Column, xattrs: bool) -> Cell {
        match *column {
            Column::Permissions    => self.render_permissions(file.permissions(), xattrs),
            Column::FileSize(fmt)  => self.render_size(file.size(), fmt),
            Column::Timestamp(t)   => self.render_time(file.timestamp(t)),
            Column::HardLinks      => self.render_links(file.links()),
            Column::Inode          => self.render_inode(file.inode()),
            Column::Blocks         => self.render_blocks(file.blocks()),
            Column::User           => self.render_user(file.user()),
            Column::Group          => self.render_group(file.group()),
            Column::GitStatus      => self.render_git_status(file.git_status()),
        }
    }

    fn render_permissions(&self, permissions: f::Permissions, xattrs: bool) -> Cell {
        let c = self.colours.perms;
        let bit = |bit, chr: &'static str, style: Style| {
            if bit { style.paint(chr) } else { self.colours.punctuation.paint("-") }
        };

        let file_type = match permissions.file_type {
            f::Type::File       => self.colours.filetypes.normal.paint("."),
            f::Type::Directory  => self.colours.filetypes.directory.paint("d"),
            f::Type::Pipe       => self.colours.filetypes.special.paint("|"),
            f::Type::Link       => self.colours.filetypes.symlink.paint("l"),
            f::Type::Special    => self.colours.filetypes.special.paint("?"),
        };

        let x_colour = if let f::Type::File = permissions.file_type { c.user_execute_file }
                                                               else { c.user_execute_other };

        let mut columns = vec![
            file_type,
            bit(permissions.user_read,     "r", c.user_read),
            bit(permissions.user_write,    "w", c.user_write),
            bit(permissions.user_execute,  "x", x_colour),
            bit(permissions.group_read,    "r", c.group_read),
            bit(permissions.group_write,   "w", c.group_write),
            bit(permissions.group_execute, "x", c.group_execute),
            bit(permissions.other_read,    "r", c.other_read),
            bit(permissions.other_write,   "w", c.other_write),
            bit(permissions.other_execute, "x", c.other_execute),
        ];

        if xattrs {
            columns.push(c.attribute.paint("@"));
        }

        Cell {
            text: ANSIStrings(&columns).to_string(),
            length: columns.len(),
        }
    }

    fn render_links(&self, links: f::Links) -> Cell {
        let style = if links.multiple { self.colours.links.multi_link_file }
                                 else { self.colours.links.normal };

        Cell::paint(style, &self.numeric.format_int(links.count))
    }

    fn render_blocks(&self, blocks: f::Blocks) -> Cell {
        match blocks {
            f::Blocks::Some(blocks)  => Cell::paint(self.colours.blocks, &blocks.to_string()),
            f::Blocks::None          => Cell::paint(self.colours.punctuation, "-"),
        }
    }

    fn render_inode(&self, inode: f::Inode) -> Cell {
        Cell::paint(self.colours.inode, &inode.0.to_string())
    }

    fn render_size(&self, size: f::Size, size_format: SizeFormat) -> Cell {
        if let f::Size::Some(offset) = size {
            let result = match size_format {
                SizeFormat::DecimalBytes  => decimal_prefix(offset as f64),
                SizeFormat::BinaryBytes   => binary_prefix(offset as f64),
                SizeFormat::JustBytes     => return Cell::paint(self.colours.size.numbers, &self.numeric.format_int(offset)),
            };

            match result {
                Standalone(bytes)    => Cell::paint(self.colours.size.numbers, &*bytes.to_string()),
                Prefixed(prefix, n)  => {
                    let number = if n < 10f64 { self.numeric.format_float(n, 1) } else { self.numeric.format_int(n as isize) };
                    let symbol = prefix.symbol();

                    Cell {
                        text: ANSIStrings( &[ self.colours.size.numbers.paint(&number[..]), self.colours.size.unit.paint(symbol) ]).to_string(),
                        length: number.len() + symbol.len(),
                    }
                }
            }
        }
        else {
            Cell::paint(self.colours.punctuation, "-")
        }
    }

    fn render_time(&self, timestamp: f::Time) -> Cell {
        let date = self.tz.at(LocalDateTime::at(timestamp.0));

        let format = if date.year() == self.current_year {
                DateFormat::parse("{2>:D} {:M} {2>:h}:{02>:m}").unwrap()
            }
            else {
                DateFormat::parse("{2>:D} {:M} {5>:Y}").unwrap()
            };

        Cell::paint(self.colours.date, &format.format(&date, &self.time))
    }

    fn render_git_status(&self, git: f::Git) -> Cell {
        Cell {
            text: ANSIStrings(&[ self.render_git_char(git.staged),
                                 self.render_git_char(git.unstaged) ]).to_string(),
            length: 2,
        }
    }

    fn render_git_char(&self, status: f::GitStatus) -> ANSIString {
        match status {
            f::GitStatus::NotModified  => self.colours.punctuation.paint("-"),
            f::GitStatus::New          => self.colours.git.new.paint("N"),
            f::GitStatus::Modified     => self.colours.git.modified.paint("M"),
            f::GitStatus::Deleted      => self.colours.git.deleted.paint("D"),
            f::GitStatus::Renamed      => self.colours.git.renamed.paint("R"),
            f::GitStatus::TypeChange   => self.colours.git.typechange.paint("T"),
        }
    }

    fn render_user(&mut self, user: f::User) -> Cell {
        let user_name = match self.users.get_user_by_uid(user.0) {
            Some(user)  => user.name,
            None        => user.0.to_string(),
        };

        let style = if self.users.get_current_uid() == user.0 { self.colours.users.user_you }
                                                         else { self.colours.users.user_someone_else };
        Cell::paint(style, &*user_name)
    }

    fn render_group(&mut self, group: f::Group) -> Cell {
        let mut style = self.colours.users.group_not_yours;

        let group_name = match self.users.get_group_by_gid(group.0) {
            Some(group) => {
                let current_uid = self.users.get_current_uid();
                if let Some(current_user) = self.users.get_user_by_uid(current_uid) {
                    if current_user.primary_group == group.gid || group.members.contains(&current_user.name) {
                        style = self.colours.users.group_yours;
                    }
                }
                group.name
            },
            None => group.0.to_string(),
        };

        Cell::paint(style, &*group_name)
    }

    /// Render the table as a vector of Cells, to be displayed on standard output.
    pub fn print_table(&self) -> Vec<Cell> {
        let mut stack = Vec::new();
        let mut cells = Vec::new();

        // Work out the list of column widths by finding the longest cell for
        // each column, then formatting each cell in that column to be the
        // width of that one.
        let column_widths: Vec<usize> = (0 .. self.columns.len())
            .map(|n| self.rows.iter().map(|row| row.column_width(n)).max().unwrap_or(0))
            .collect();

        let total_width: usize = self.columns.len() + column_widths.iter().sum::<usize>();

        for row in self.rows.iter() {
            let mut cell = Cell::empty();

            if let Some(ref cells) = row.cells {
                for (n, width) in column_widths.iter().enumerate() {
                    match self.columns[n].alignment() {
                        Alignment::Left  => { cell.append(&cells[n]); cell.add_spaces(width - cells[n].length); }
                        Alignment::Right => { cell.add_spaces(width - cells[n].length); cell.append(&cells[n]); }
                    }

                    cell.add_spaces(1);
                }
            }
            else {
                cell.add_spaces(total_width)
            }

            let mut filename = String::new();
            let mut filename_length = 0;

            // A stack tracks which tree characters should be printed. It's
            // necessary to maintain information about the previously-printed
            // lines, as the output will change based on whether the
            // *previous* entry was the last in its directory.
            stack.resize(row.depth + 1, TreePart::Edge);
            stack[row.depth] = if row.last { TreePart::Corner } else { TreePart::Edge };

            for i in 1 .. row.depth + 1 {
                filename.push_str(&*self.colours.punctuation.paint(stack[i].ascii_art()).to_string());
                filename_length += 4;
            }

            stack[row.depth] = if row.last { TreePart::Blank } else { TreePart::Line };

            // If any tree characters have been printed, then add an extra
            // space, which makes the output look much better.
            if row.depth != 0 {
                filename.push(' ');
                filename_length += 1;
            }

            // Print the name without worrying about padding.
            filename.push_str(&*row.name.text);
            filename_length += row.name.length;

            cell.append(&Cell { text: filename, length: filename_length });
            cells.push(cell);
        }

        cells
    }
}


#[derive(PartialEq, Debug, Clone)]
enum TreePart {

    /// Rightmost column, *not* the last in the directory.
    Edge,

    /// Not the rightmost column, and the directory has not finished yet.
    Line,

    /// Rightmost column, and the last in the directory.
    Corner,

    /// Not the rightmost column, and the directory *has* finished.
    Blank,
}

impl TreePart {
    fn ascii_art(&self) -> &'static str {
        match *self {
            TreePart::Edge    => "├──",
            TreePart::Line    => "│  ",
            TreePart::Corner  => "└──",
            TreePart::Blank   => "   ",
        }
    }
}


#[cfg(test)]
pub mod test {
    pub use super::Table;
    pub use file::File;
    pub use file::fields as f;

    pub use column::{Cell, Column};

    pub use users::{User, Group, uid_t, gid_t};
    pub use users::mock::MockUsers;

    pub use ansi_term::Style;
    pub use ansi_term::Colour::*;

    pub fn newser(uid: uid_t, name: &str, group: gid_t) -> User {
        User {
            uid: uid,
            name: name.to_string(),
            primary_group: group,
            home_dir: String::new(),
            shell: String::new(),
        }
    }

    // These tests create a new, default Table object, then fill in the
    // expected style in a certain way. This means we can check that the
    // right style is being used, as otherwise, it would just be plain.
    //
    // Doing things with fields is way easier than having to fake the entire
    // Metadata struct, which is what I was doing before!

    mod users {
        #![allow(unused_results)]
        use super::*;

        #[test]
        fn named() {
            let mut table = Table::default();
            table.colours.users.user_you = Red.bold();

            let mut users = MockUsers::with_current_uid(1000);
            users.add_user(newser(1000, "enoch", 100));
            table.users = users;

            let user = f::User(1000);
            let expected = Cell::paint(Red.bold(), "enoch");
            assert_eq!(expected, table.render_user(user))
        }

        #[test]
        fn unnamed() {
            let mut table = Table::default();
            table.colours.users.user_you = Cyan.bold();

            let users = MockUsers::with_current_uid(1000);
            table.users = users;

            let user = f::User(1000);
            let expected = Cell::paint(Cyan.bold(), "1000");
            assert_eq!(expected, table.render_user(user));
        }

        #[test]
        fn different_named() {
            let mut table = Table::default();
            table.colours.users.user_someone_else = Green.bold();
            table.users.add_user(newser(1000, "enoch", 100));

            let user = f::User(1000);
            let expected = Cell::paint(Green.bold(), "enoch");
            assert_eq!(expected, table.render_user(user));
        }

        #[test]
        fn different_unnamed() {
            let mut table = Table::default();
            table.colours.users.user_someone_else = Red.normal();

            let user = f::User(1000);
            let expected = Cell::paint(Red.normal(), "1000");
            assert_eq!(expected, table.render_user(user));
        }

        #[test]
        fn overflow() {
            let mut table = Table::default();
            table.colours.users.user_someone_else = Blue.underline();

            let user = f::User(2_147_483_648);
            let expected = Cell::paint(Blue.underline(), "2147483648");
            assert_eq!(expected, table.render_user(user));
        }
    }

    mod groups {
        #![allow(unused_results)]
        use super::*;

        #[test]
        fn named() {
            let mut table = Table::default();
            table.colours.users.group_not_yours = Fixed(101).normal();

            let mut users = MockUsers::with_current_uid(1000);
            users.add_group(Group { gid: 100, name: "folk".to_string(), members: vec![] });
            table.users = users;

            let group = f::Group(100);
            let expected = Cell::paint(Fixed(101).normal(), "folk");
            assert_eq!(expected, table.render_group(group))
        }

        #[test]
        fn unnamed() {
            let mut table = Table::default();
            table.colours.users.group_not_yours = Fixed(87).normal();

            let users = MockUsers::with_current_uid(1000);
            table.users = users;

            let group = f::Group(100);
            let expected = Cell::paint(Fixed(87).normal(), "100");
            assert_eq!(expected, table.render_group(group));
        }

        #[test]
        fn primary() {
            let mut table = Table::default();
            table.colours.users.group_yours = Fixed(64).normal();

            let mut users = MockUsers::with_current_uid(2);
            users.add_user(newser(2, "eve", 100));
            users.add_group(Group { gid: 100, name: "folk".to_string(), members: vec![] });
            table.users = users;

            let group = f::Group(100);
            let expected = Cell::paint(Fixed(64).normal(), "folk");
            assert_eq!(expected, table.render_group(group))
        }

        #[test]
        fn secondary() {
            let mut table = Table::default();
            table.colours.users.group_yours = Fixed(31).normal();

            let mut users = MockUsers::with_current_uid(2);
            users.add_user(newser(2, "eve", 666));
            users.add_group(Group { gid: 100, name: "folk".to_string(), members: vec![ "eve".to_string() ] });
            table.users = users;

            let group = f::Group(100);
            let expected = Cell::paint(Fixed(31).normal(), "folk");
            assert_eq!(expected, table.render_group(group))
        }

        #[test]
        fn overflow() {
            let mut table = Table::default();
            table.colours.users.group_not_yours = Blue.underline();

            let group = f::Group(2_147_483_648);
            let expected = Cell::paint(Blue.underline(), "2147483648");
            assert_eq!(expected, table.render_group(group));
        }
    }
}
