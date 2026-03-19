

fn main() {
    let guess = "42".parse::<u32>().expect("Not a number!");
    println!("The value of guess is {guess}");
}