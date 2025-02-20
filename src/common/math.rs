use crate::defaults::UnsignedIntegerType;

// Euclidean algorithm for GCD.
pub(crate) fn gcd(a: UnsignedIntegerType, b: UnsignedIntegerType) -> UnsignedIntegerType {
    if b == 0 { a } else { gcd(b, a % b) }
}

// LCM computed via GCD.
pub(crate) fn lcm(a: UnsignedIntegerType, b: UnsignedIntegerType) -> UnsignedIntegerType {
    a / gcd(a, b) * b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcd_basic() {
        assert_eq!(gcd(10, 5), 5);
        assert_eq!(gcd(5, 10), 5);
        assert_eq!(gcd(7, 3), 1);
        assert_eq!(gcd(14, 21), 7);
    }

    #[test]
    fn test_gcd_with_zero() {
        // By definition, gcd(0, n) = n and gcd(n, 0) = n.
        assert_eq!(gcd(0, 5), 5);
        assert_eq!(gcd(5, 0), 5);
        // Here, we define gcd(0, 0) as 0.
        assert_eq!(gcd(0, 0), 0);
    }

    #[test]
    fn test_lcm_basic() {
        assert_eq!(lcm(10, 5), 10);
        assert_eq!(lcm(5, 10), 10);
        assert_eq!(lcm(7, 3), 21);
        assert_eq!(lcm(14, 21), 42);
    }

    #[test]
    fn test_lcm_with_zero() {
        // lcm(0, n) and lcm(n, 0) yield 0.
        assert_eq!(lcm(0, 5), 0);
        assert_eq!(lcm(5, 0), 0);
    }
}
