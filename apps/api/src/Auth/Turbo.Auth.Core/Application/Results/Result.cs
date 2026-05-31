namespace Turboapi.Auth.Application.Results
{
    /// <summary>
    /// Base class for results, indicating success or failure with an error.
    /// </summary>
    /// <typeparam name="TError">The type of the error enum or class.</typeparam>
    public interface IResult {
        bool IsSuccess { get; }
    }
    
    public abstract class ResultBase<TError> : IResult where TError : notnull
    {
        public bool IsSuccess { get; }
        public bool IsFailure => !IsSuccess;
        public TError? Error { get; }

        protected ResultBase(bool isSuccess, TError? error)
        {
            if (isSuccess && error != null && !error.Equals(default(TError))) // For enums, default is usually 0
            {
                // For reference types (error classes), default is null.
                // For enums, ensure error is default if successful.
                if (typeof(TError).IsEnum && !error.Equals(Activator.CreateInstance(typeof(TError))))
                {
                     throw new InvalidOperationException("Error must be default for a successful result if TError is an enum.");
                }
                else if (!typeof(TError).IsEnum && error != null)
                {
                     throw new InvalidOperationException("Error must be null for a successful result if TError is a reference type.");
                }
            }
            if (!isSuccess && error == null)
            {
                 throw new InvalidOperationException("Error cannot be null for a failed result.");
            }
            if (!isSuccess && typeof(TError).IsEnum && error!.Equals(Activator.CreateInstance(typeof(TError))))
            {
                 throw new InvalidOperationException("Error cannot be the default enum value for a failed result.");
            }


            IsSuccess = isSuccess;
            Error = error;
        }
    }

    /// <summary>
    /// Represents the result of an operation that returns a value on success.
    /// </summary>
    /// <typeparam name="TSuccess">The type of the success value.</typeparam>
    /// <typeparam name="TError">The type of the error enum or class.</typeparam>
    public class Result<TSuccess, TError> : ResultBase<TError> where TError : notnull
    {
        public TSuccess? Value { get; }

        private Result(TSuccess value) : base(true, default) // default(TError) is null for classes, 0 for enums
        {
            Value = value;
        }

        private Result(TError error) : base(false, error)
        {
            Value = default; // Default for TSuccess (null for reference types, 0 for value types)
        }

        public static implicit operator Result<TSuccess, TError>(TSuccess value)
        {
            // Disallow implicit conversion from null if TSuccess is non-nullable,
            // though C# 8+ nullable reference types handle this better at compile time.
            // if (value == null && default(TSuccess) != null) 
            //    throw new ArgumentNullException(nameof(value), "Cannot create a success result with a null value for a non-nullable type.");
            return new Result<TSuccess, TError>(value);
        }

        public static implicit operator Result<TSuccess, TError>(TError error)
        {
            // if (error == null) // TError is constrained to notnull
            //    throw new ArgumentNullException(nameof(error), "Cannot create a failure result with a null error.");
            return new Result<TSuccess, TError>(error);
        }

        /// <summary>
        /// Executes one of the provided functions based on the result state.
        /// </summary>
        /// <typeparam name="TResult">The return type of the match functions.</typeparam>
        /// <param name="success">Function to execute if the result is successful.</param>
        /// <param name="failure">Function to execute if the result is a failure.</param>
        /// <returns>The value returned by the executed function.</returns>
        public TResult Match<TResult>(Func<TSuccess, TResult> success, Func<TError, TResult> failure)
        {
            if (success == null) throw new ArgumentNullException(nameof(success));
            if (failure == null) throw new ArgumentNullException(nameof(failure));

            return IsSuccess && Value != null // Check Value for null if TSuccess can be null
                ? success(Value)
                : failure(Error!); // Error is guaranteed non-null on failure
        }

        /// <summary>
        /// Executes one of the provided actions based on the result state.
        /// </summary>
        /// <param name="success">Action to execute if the result is successful.</param>
        /// <param name="failure">Action to execute if the result is a failure.</param>
        public void Switch(Action<TSuccess> success, Action<TError> failure)
        {
            if (success == null) throw new ArgumentNullException(nameof(success));
            if (failure == null) throw new ArgumentNullException(nameof(failure));

            if (IsSuccess && Value != null)
            {
                success(Value);
            }
            else
            {
                failure(Error!);
            }
        }
    }

    /// <summary>
    /// Represents the result of an operation that does not return a value on success (void-like).
    /// It still carries an error type for failure cases.
    /// </summary>
    /// <typeparam name="TError">The type of the error enum or class.</typeparam>
    public class Result<TError> : ResultBase<TError> where TError : notnull
    {
        private static readonly Result<TError> _successInstance = new Result<TError>();

        private Result() : base(true, default) // Success, no specific error
        {
        }

        private Result(TError error) : base(false, error)
        {
        }

        /// <summary>
        /// Creates a success result.
        /// </summary>
        public static Result<TError> Success() => _successInstance;
        
        /// <summary>
        /// Creates a failure result with the specified error.
        /// </summary>
        public static Result<TError> Failure(TError error)
        {
            // if (error == null) // TError is constrained to notnull
            //    throw new ArgumentNullException(nameof(error), "Cannot create a failure result with a null error.");
            return new Result<TError>(error);
        }


        public static implicit operator Result<TError>(TError error) => Failure(error);

        /// <summary>
        /// Executes one of the provided functions based on the result state.
        /// </summary>
        /// <typeparam name="TResult">The return type of the match functions.</typeparam>
        /// <param name="success">Function to execute if the result is successful.</param>
        /// <param name="failure">Function to execute if the result is a failure.</param>
        /// <returns>The value returned by the executed function.</returns>
        public TResult Match<TResult>(Func<TResult> success, Func<TError, TResult> failure)
        {
            if (success == null) throw new ArgumentNullException(nameof(success));
            if (failure == null) throw new ArgumentNullException(nameof(failure));

            return IsSuccess
                ? success()
                : failure(Error!); // Error is guaranteed non-null on failure
        }

        /// <summary>
        /// Executes one of the provided actions based on the result state.
        /// </summary>
        /// <param name="success">Action to execute if the result is successful.</param>
        /// <param name="failure">Action to execute if the result is a failure.</param>
        public void Switch(Action success, Action<TError> failure)
        {
            if (success == null) throw new ArgumentNullException(nameof(success));
            if (failure == null) throw new ArgumentNullException(nameof(failure));

            if (IsSuccess)
            {
                success();
            }
            else
            {
                failure(Error!);
            }
        }
    }

    /// <summary>
    /// Provides static factory methods for creating Result instances.
    /// </summary>
    public static class Result
    {
        /// <summary>
        /// Creates a success result with a value.
        /// </summary>
        public static Result<TSuccess, TError> Success<TSuccess, TError>(TSuccess value) where TError : notnull
            => value; // Uses implicit conversion

        /// <summary>
        /// Creates a success result for an operation that returns no specific value (void-like).
        /// </summary>
        public static Result<TError> Success<TError>() where TError : notnull
            => Results.Result<TError>.Success();

        /// <summary>
        /// Creates a failure result.
        /// </summary>
        public static Result<TSuccess, TError> Failure<TSuccess, TError>(TError error) where TError : notnull
            => error; // Uses implicit conversion
        
        /// <summary>
        /// Creates a failure result for an operation that returns no specific value on success (void-like).
        /// </summary>
        public static Result<TError> Failure<TError>(TError error) where TError : notnull
            => Results.Result<TError>.Failure(error);
    }
}