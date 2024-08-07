use rand::Rng;

fn main() {
  let random_number: u32 = rand::thread_rng().gen_range(1..=100);

  println!(
    "Here's a random number between 1 and 100: {}",
    random_number
  );
}
