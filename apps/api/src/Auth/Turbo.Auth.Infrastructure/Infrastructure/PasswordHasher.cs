using System.Security.Cryptography;
using Microsoft.AspNetCore.Cryptography.KeyDerivation;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Infrastructure.Auth
{
    public class PasswordHasher : IPasswordHasher
    {
        public string HashPassword(string password)
        {
            if (string.IsNullOrEmpty(password))
            {
                throw new ArgumentNullException(nameof(password), "Password cannot be null or empty.");
            }

            byte[] salt = new byte[128 / 8];
            using (var rng = RandomNumberGenerator.Create())
            {
                rng.GetBytes(salt);
            }

            string hashed = Convert.ToBase64String(KeyDerivation.Pbkdf2(
                password: password,
                salt: salt,
                prf: KeyDerivationPrf.HMACSHA256,
                iterationCount: 100000,
                numBytesRequested: 256 / 8));

            return $"{Convert.ToBase64String(salt)}.{hashed}";
        }

        public bool VerifyPassword(string password, string hashedPassword)
        {
            if (string.IsNullOrEmpty(password))
            {
                // Depending on security policy, might return false or throw.
                // Returning false is common to avoid leaking information about validation process.
                return false; 
            }
            if (string.IsNullOrEmpty(hashedPassword))
            {
                return false;
            }

            var parts = hashedPassword.Split('.', 2); // Split into exactly 2 parts
            if (parts.Length != 2)
            {
                // Invalid format for stored hash
                return false; 
            }

            try
            {
                var salt = Convert.FromBase64String(parts[0]);
                var hash = parts[1];

                string hashed = Convert.ToBase64String(KeyDerivation.Pbkdf2(
                    password: password,
                    salt: salt,
                    prf: KeyDerivationPrf.HMACSHA256,
                    iterationCount: 100000,
                    numBytesRequested: 256 / 8));

                return CryptographicOperations.FixedTimeEquals(
                    System.Text.Encoding.UTF8.GetBytes(hash), 
                    System.Text.Encoding.UTF8.GetBytes(hashed)
                );
            }
            catch (FormatException)
            {
                // Salt was not a valid Base64 string
                return false;
            }
            catch (ArgumentNullException)
            {
                // Should not happen if initial checks pass, but good for robustness
                return false;
            }
        }
    }
}