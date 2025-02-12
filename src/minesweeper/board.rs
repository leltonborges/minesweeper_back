use std::collections::BTreeSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use paperclip::actix::Apiv2Schema;
use rand::{Rng, thread_rng};
use serde::{Deserialize, Serialize};
use serde_valid::Validate;

use crate::constant::MOVEMENTS;
use crate::minesweeper::board::CellContent::{Block, Free, Hint, Mine, Treasure};
use crate::property::{Config, MinesweeperDefault};

#[derive(Debug, Clone, Serialize, Deserialize, Apiv2Schema, Ord, Eq, PartialOrd, PartialEq)]
pub enum CellContent {
    Treasure,
    Mine,
    Block,
    Hint(String),
    Free,
}

#[derive(Debug, Clone, Serialize, Deserialize, Apiv2Schema, Ord, PartialOrd, Eq, PartialEq)]
pub struct Cell {
    pub content: CellContent,
    pub row: usize,
    pub col: usize,
    pub revealed: bool,
    pub cell_num_mines: usize,
    pub cell_num_hints: usize,
    pub cell_num_block: usize,
    pub neighbors: BTreeSet<Cell>
}

#[derive(Debug, Clone, Serialize, Deserialize, Apiv2Schema)]
struct Move {
    row: usize,
    col: usize,
    action: MoveAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, Apiv2Schema)]
enum MoveAction {
    Reveal,
    MarkMine,
    UnmarkMine,
}

#[derive(Debug, Serialize, Deserialize, Apiv2Schema)]
pub struct Minesweeper {
    pub grid: Vec<Vec<Cell>>,
    pub current: Cell,
    pub steps: usize,
    pub paths: BTreeSet<Cell>,
    pub config: ConfigMinesweeper,
    pub timer: GameTimer,
}

#[derive(Debug, Clone, Validate, Serialize, Deserialize, Apiv2Schema)]
pub struct ConfigMinesweeper {
    #[validate(minimum = 3, message = "Minimum is 3.")]
    #[validate(maximum = 10, message = "Maximum is 10")]
    pub width: usize,
    #[validate(minimum = 3, message = "Minimum is 3.")]
    #[validate(maximum = 10, message = "Maximum is 10")]
    pub height: usize,
    pub num_mines: usize,
    pub num_hints: usize,
    pub num_blocks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Apiv2Schema)]
pub struct GameTimer {
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
}

impl Clone for Minesweeper {
    fn clone(&self) -> Self {
        Minesweeper {
            grid: self.grid.clone(),
            current: self.current.clone(),
            steps: self.steps,
            paths: self.paths.clone(),
            config: self.config.clone(),
            timer: self.timer.clone(),
        }
    }
}

impl Cell {
    pub fn new(content: CellContent, row: usize, col: usize) -> Self {
        Cell {
            content,
            row,
            col,
            revealed: false,
            cell_num_mines: 0,
            cell_num_hints: 0,
            cell_num_block: 0,
            neighbors: BTreeSet::new()
        }
    }

    pub fn new_free(row: usize, col: usize) -> Self {
        Cell::new(Free, row, col)
    }

    fn reveal(&mut self) -> Self {
        self.revealed = true;
        self.to_owned()
    }
}

impl GameTimer {
    pub fn new() -> Self {
        GameTimer {
            start_time: None,
            end_time: None,
        }
    }
    pub fn start(&mut self) {
        self.start_time = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
    }

    pub fn stop(&mut self) {
        self.end_time = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
    }

    pub fn elapsed_time(&self) -> Option<Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => {
                let start_instant = UNIX_EPOCH + Duration::from_secs(start);
                let end_instant = UNIX_EPOCH + Duration::from_secs(end);
                Some(end_instant.duration_since(start_instant).unwrap_or_else(|_| Duration::default()))
            }
            _ => None,
        }
    }
}

impl Minesweeper {
    pub fn new(grid: Vec<Vec<Cell>>,
           start: Cell,
           paths: BTreeSet<Cell>,
           config: ConfigMinesweeper,
           timer: GameTimer) -> Self {
        Minesweeper {
            grid,
            current: start,
            steps: 0,
            paths,
            config,
            timer,
        }
    }

    fn get_start() -> Cell {
        let properties = Config::get_properties().minesweeper();
        let mut cell = Cell::new_free(properties.start.row, properties.start.col);
        cell.reveal()
    }

    pub fn new_random(config: ConfigMinesweeper) -> Self {
        let grid = create_grid(config.height, config.width);

        let timer = GameTimer {
            start_time: None,
            end_time: None,
        };

        let mut game = Minesweeper {
            grid,
            current: Self::get_start(),
            steps: 0,
            paths: BTreeSet::new(),
            config,
            timer,
        };

        game.random_mines().random_blocks().place_treasure(None);

        game
    }

    fn apply_random_content(&mut self, cell_content: CellContent, total_num: usize) {
        let mut rng = thread_rng();
        let mut content_added = 0;
        let mut attempts = 0;
        let free_cells = self.grid.iter().flat_map(|row| row.iter()).filter(|cell| cell.content == Free).count();

        if total_num > free_cells {
            println!("Não é possível adicionar {} itens de conteúdo; apenas {} células livres disponíveis.", total_num, free_cells);
            return;
        }

        while content_added < total_num && attempts < total_num * 10 {
            let row = rng.gen_range(0..self.config.height);
            let col = rng.gen_range(0..self.config.width);
            if self.grid[row][col].content == Free {
                self.apply_values(row, col, cell_content.clone());
                content_added += 1;
            }
            attempts += 1;
        }

        if content_added < total_num {
            println!("Falha ao adicionar todos os itens: {}/{}", content_added, total_num);
        }
    }

    fn random_mines(&mut self) -> &mut Self {
        self.apply_random_content(Mine, self.config.num_mines);
        self
    }

    fn random_blocks(&mut self) -> &mut Self {
        self.apply_random_content(Block, self.config.num_blocks);
        self
    }

    fn place_treasure_at(&mut self, place: (usize, usize)) {
        if self.is_valid_position(place.0, place.1) {
            self.grid[place.0][place.1].content = Treasure;
            self.add_hints_around(place.0, place.1);
        }
    }

    fn place_random_treasure(&mut self) {
        let mut rng = thread_rng();
        'outer: loop {
            let row = rng.gen_range(0..self.config.height);
            let col = rng.gen_range(0..self.config.width);
            if self.grid[row][col].content == Free {
                self.grid[row][col].content = Treasure;
                self.add_hints_around(row, col);
                break 'outer;
            }
        }
    }

    fn place_treasure(&mut self, place_treasure: Option<(usize, usize)>) -> &mut Self {
        match place_treasure {
            Some(place) => self.place_treasure_at(place),
            None => self.place_random_treasure(),
        }
        self
    }

    fn add_hints_around(&mut self, row: usize, col: usize) {
        for &(dx, dy) in MOVEMENTS.iter() {
            let (new_row, new_col) = ((row as isize + dx) as usize, (col as isize + dy) as usize);
            if self.is_valid_position(new_row, new_col) {
                self.add_hint(new_row, new_col);
            }
        }
    }

    fn add_hint(&mut self, row: usize, col: usize) {
        if self.grid[row][col].content == Free {
            self.grid[row][col].content = Hint("Perto do tesouro".to_string());
            self.grid[row][col].cell_num_hints += 1;
        }
    }

    fn apply_values(&mut self, row: usize, col: usize, cell_content: CellContent) {
        self.grid[row][col].content = cell_content.clone();
        for &(dx, dy) in MOVEMENTS.iter() {
            let (new_row, new_col) = ((row as isize + dx) as usize, (col as isize + dy) as usize);
            if self.is_valid_position(new_row, new_col) {
                self.set_neighbor_count(cell_content.clone(), new_row, new_col);
            }
        }
    }

    fn set_neighbor_count(&mut self, cell_content: CellContent, new_row: usize, new_col: usize) {
        match cell_content {
            Mine => self.grid[new_row][new_col].cell_num_mines += 1,
            Block => self.grid[new_row][new_col].cell_num_block += 1,
            _ => {}
        }
    }

    fn is_valid_position(&self, row: usize, col: usize) -> bool {
        return row < self.config.height && col < self.config.width;
    }

    fn is_valid_movement(&self, new_row: usize, new_col: usize) -> bool {
        if !self.is_valid_position(new_row, new_col) {
            return false;
        }

        let Cell { row, col, .. } = self.current;
        let sub_row = row as i8 - new_row as i8;
        let sub_col = col as i8 - new_col as i8;
        sub_row.abs() <= 1 && sub_col.abs() <= 1 && (sub_row != 0 || sub_col != 0)
    }

    pub fn move_to(&mut self, new_row: usize, new_col: usize) {
        if self.is_valid_movement(new_row, new_col) {
            self.current = self.grid[new_row][new_col].clone();
            self.steps += 1;
            self.paths.insert(self.current.clone());
            self.grid[new_row][new_col].revealed = true;
        }
    }
}

fn create_grid(height: usize, width: usize) -> Vec<Vec<Cell>> {
    let mut grid: Vec<Vec<Cell>> = Vec::new();
    for row in 0..height {
        let mut row_vec: Vec<Cell> = Vec::new();
        for col in 0..width {
            row_vec.push(Cell::new_free(row, col));
        }
        grid.push(row_vec);
    }
    grid
}

impl ConfigMinesweeper {
    pub fn from(default: MinesweeperDefault) -> Self {
        Self {
            width: default.width,
            height: default.height,
            num_mines: default.num_mines,
            num_blocks: default.num_blocks,
            num_hints: default.num_hints,
        }
    }
}