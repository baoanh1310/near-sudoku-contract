use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    log,
    serde::{Deserialize, Serialize},
    AccountId, PanicOnDefault, Promise,
};
use near_sdk::{env, near_bindgen};

// 1 Ⓝ in yoctoNEAR
const PRIZE_AMOUNT: u128 = 1_000_000_000_000_000_000_000_000;

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct Puzzle {
    status: PuzzleStatus,  
    initial: String,
}

#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize, Debug)]
#[serde(crate = "near_sdk::serde")]
pub enum PuzzleStatus {
    Unsolved,
    Solved { memo: String },
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct UnsolvedPuzzles {
    puzzles: Vec<JsonPuzzle>,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct JsonPuzzle {
    /// The human-readable (not in bytes) hash of the solution
    solution_hash: String,
    status: PuzzleStatus,
    initial: String,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Sudoku {
    owner_id: AccountId,
    /// { solution_hash: puzzle_status }
    puzzles: LookupMap<String, Puzzle>, 
    /// set of solution_hashes
    unsolved_puzzles: UnorderedSet<String>, 
}

#[near_bindgen]
impl Sudoku {

    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            puzzles: LookupMap::new(b"c"),
            unsolved_puzzles: UnorderedSet::new(b"u"),
        }
    }

    pub fn submit_solution(&mut self, solution: String, memo: String) {
        let hashed_input = env::sha256(solution.as_bytes());
        let hashed_input_hex = hex::encode(&hashed_input);

        // Check to see if the hashed answer is among the puzzles
        let mut puzzle = self
            .puzzles
            .get(&hashed_input_hex)
            .expect("ERR_NOT_CORRECT_ANSWER");

        // Check if the puzzle is already solved. If it's unsolved, set the status to solved,
        //   then proceed to update the puzzle and pay the winner.
        puzzle.status = match puzzle.status {
            PuzzleStatus::Unsolved => PuzzleStatus::Solved { memo: memo.clone() },
            _ => {
                env::panic_str("ERR_PUZZLE_SOLVED");
            }
        };

        // Reinsert the puzzle back in after we modified the status:
        self.puzzles.insert(&hashed_input_hex, &puzzle);
        // Remove from the list of unsolved ones
        self.unsolved_puzzles.remove(&hashed_input_hex);

        log!(
            "Puzzle with solution hash {} solved, with memo: {}",
            hashed_input_hex,
            memo
        );

        // Transfer the prize money to the winner
        Promise::new(env::predecessor_account_id()).transfer(PRIZE_AMOUNT);
    }

    /// Get the hash of a sudoku puzzle solution from the unsolved_puzzles
    pub fn get_solution(&self, puzzle_index: u32) -> Option<String> {
        let mut index = 0;
        for puzzle_hash in self.unsolved_puzzles.iter() {
            if puzzle_index == index {
                return Some(puzzle_hash);
            }
            index += 1;
        }
        // Did not find that index
        None
    }

    pub fn get_puzzle_status(&self, solution_hash: String) -> Option<PuzzleStatus> {
        let puzzle = self.puzzles.get(&solution_hash);
        if puzzle.is_none() {
            return None;
        }
        Some(puzzle.unwrap().status)
    }

    pub fn new_puzzle(&mut self, solution_hash: String, initial: String) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Only the owner may call this method"
        );
        let existing = self.puzzles.insert(
            &solution_hash,
            &Puzzle {
                status: PuzzleStatus::Unsolved,
                initial: initial,
            },
        );

        assert!(existing.is_none(), "Puzzle with that key already exists");
        self.unsolved_puzzles.insert(&solution_hash);
    }

    pub fn get_unsolved_puzzles(&self) -> UnsolvedPuzzles {
        let solution_hashes = self.unsolved_puzzles.to_vec();
        let mut all_unsolved_puzzles = vec![];
        for hash in solution_hashes {
            let puzzle = self
                .puzzles
                .get(&hash)
                .unwrap_or_else(|| env::panic_str("ERR_LOADING_PUZZLE"));
            let json_puzzle = JsonPuzzle {
                solution_hash: hash,
                status: puzzle.status,
                initial: puzzle.initial,
            };
            all_unsolved_puzzles.push(json_puzzle)
        }
        UnsolvedPuzzles {
            puzzles: all_unsolved_puzzles,
        }
    }
}

// use the attribute below for unit tests
#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{testing_env, AccountId};

    // part of writing unit tests is setting up a mock context
    // provide a `predecessor` here, it'll modify the default context
    fn get_context(predecessor: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(predecessor);
        builder
    }

    #[test]
    fn debug_get_hash() {
        // Basic set up for a unit test
        testing_env!(VMContextBuilder::new().build());

        // Using a unit test to rapidly debug and iterate
        let debug_solution = "417369825632158947958724316825437169791586432346912758289643571573291684164875293";
        let debug_hash_bytes = env::sha256(debug_solution.as_bytes());
        let debug_hash_string = hex::encode(debug_hash_bytes);
        println!("Let's debug: {:?}", debug_hash_string);
    }

    #[test]
    #[should_panic(expected = "ERR_NOT_CORRECT_ANSWER")]
    fn check_submit_solution_failure() {
        // Get Alice as an account ID
        let alice = AccountId::new_unchecked("alice.testnet".to_string());
        // Set up the testing context and unit test environment
        let context = get_context(alice.clone());
        testing_env!(context.build());

        // Set up contract object and call the new method
        let mut contract = Sudoku::new(alice);
        // Add puzzle
        contract.new_puzzle(
            "55c0b3434cfb2aeb07caece44e8d3a69856f6d95e22c5e61ec7e46075fc5e5e6".to_string(),
            "417.6982.632158947958724.16825437169791586432346912758289643571573291684164875..3".to_string(),
        );
        contract.submit_solution("427369825632158947958724316825437169791586432346912758289643571573291684164875293".to_string(), "my memo".to_string());
    }

    #[test]
    fn check_submit_solution_success() {
        // Get Alice as an account ID
        let alice = AccountId::new_unchecked("alice.testnet".to_string());
        // Set up the testing context and unit test environment
        let context = get_context(alice.clone());
        testing_env!(context.build());

        // Set up contract object
        let mut contract = Sudoku::new(alice);

        // Add puzzle
        contract.new_puzzle(
            "55c0b3434cfb2aeb07caece44e8d3a69856f6d95e22c5e61ec7e46075fc5e5e6".to_string(),
            "417.6982.632158947958724.16825437169791586432346912758289643571573291684164875..3".to_string(),
        );

        contract.submit_solution(
            "417369825632158947958724316825437169791586432346912758289643571573291684164875293".to_string(),
            "my memo".to_string(),
        );

        // Ensure the puzzle status is now Solved
        // contract.get_puzzle_status("55c0b3434cfb2aeb07caece44e8d3a69856f6d95e22c5e61ec7e46075fc5e5e6".to_string());
        // assert_eq!(contract.unsolved_puzzles.len(), 0, "Should not have any unsolved puzzles.");
    }
}