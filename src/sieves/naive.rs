pub fn eratosthenes_naive(limit: usize) -> Vec<usize> {
    let mut primes = Vec::new();
    let mut nums = vec![true; limit];
    nums[0] = false;
    nums[1] = false;
    let mut counter = 4;
    let sqrt = (limit as f64).sqrt() as usize;
    let mut invalidating = 2;
    while invalidating <= sqrt {
        while counter < nums.len() {
            nums[counter] = false;
            counter += invalidating;
        }
        invalidating += 1;
        while !nums[invalidating] {
            invalidating += 1;
        }
        counter = invalidating * 2
    }
    for n in 0..nums.len() - 1 {
        if nums[n] {
            primes.push(n);
        }
    }
    primes
}
