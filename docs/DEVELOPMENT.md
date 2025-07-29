# Development Setup

This project uses Deno's built-in tools for linting, formatting, and type checking, plus a custom lint-staged implementation.

## Git Hooks Setup

To set up Git hooks for automatic code formatting before commits:

```bash
deno task setup-hooks
```

This will install a pre-commit hook that automatically processes staged files using our lint-staged approach.

## Available Tasks

### Code Quality
- `deno task format` - Format code using Deno's formatter
- `deno task format:check` - Check if code is properly formatted
- `deno task lint` - Run Deno's linter
- `deno task lint:fix` - Run linter with auto-fix
- `deno task type-check` - Run TypeScript type checking
- `deno task lint-staged` - Run checks only on staged files (like lint-staged)
- `deno task pre-commit` - Pre-commit checks (uses lint-staged)
- `deno task ci` - Run all CI checks (format check, lint, type-check, tests)

### Development
- `deno task start` - Start development server
- `deno task build` - Build for production
- `deno task preview` - Preview production build

### Testing
- `deno task test` - Run all tests
- `deno task test:watch` - Run tests in watch mode
- `deno task test:coverage` - Run tests with coverage

## Lint-Staged for Deno

We've implemented a custom lint-staged equivalent for Deno that processes only staged files:

### Features
- **Staged Files Only**: Only processes files that are staged for commit
- **Automatic Formatting**: Formats staged files and re-stages them
- **Type Checking**: Runs TypeScript type checking on staged files
- **Linting**: Runs Deno's linter (non-blocking warnings)
- **Fast**: Only processes changed files, not the entire codebase

### Usage

```bash
# Run lint-staged manually
deno task lint-staged

# Automatically runs via pre-commit hook
git commit -m "your message"
```

### What it does:
1. **Identifies staged files**: Gets list of staged TypeScript/JavaScript files
2. **Formats files**: Runs `deno fmt` on staged files
3. **Re-stages formatted files**: Adds formatted files back to staging
4. **Runs linting**: Checks for linting issues (warnings only)
5. **Type checks**: Runs TypeScript type checking (blocking)

## Pre-commit Hook

The pre-commit hook will:
1. Run lint-staged on all staged TypeScript/JavaScript files
2. Format code automatically
3. Check types and fail if there are type errors
4. Show linting warnings but not fail the commit

If you want to bypass the pre-commit hook (not recommended), you can use:
```bash
git commit --no-verify -m "your message"
```

## Manual Code Quality Checks

Before pushing, it's recommended to run:
```bash
deno task ci
```

This will run all the checks that would run in CI/CD.

## Configuration

- **Formatting**: Configured in `deno.json` under the `fmt` section
- **Linting**: Configured in `deno.json` under the `lint` section
- **Git Hooks**: Located in `.git/hooks/` after running setup
- **Lint-staged**: Custom implementation in `scripts/lint-staged.ts`

## Why Deno Tools + Custom Lint-Staged?

We use Deno's built-in tools with our custom lint-staged implementation because:

### Deno Tools Benefits:
- **Zero Configuration**: Works out of the box with sensible defaults
- **Fast**: Built in Rust, significantly faster than Node.js alternatives
- **Consistent**: Same formatting and linting rules across all Deno projects
- **Integrated**: Seamlessly works with Deno's type checker and runtime
- **No Dependencies**: No need to manage additional npm packages

### Custom Lint-Staged Benefits:
- **Performance**: Only processes staged files, not entire codebase
- **Deno Native**: Built specifically for Deno's toolchain
- **Flexible**: Easy to customize for project-specific needs
- **Type Safe**: Written in TypeScript with proper error handling

## Troubleshooting

### Git Hook Not Working
If the pre-commit hook isn't working:
1. Make sure you ran `deno task setup-hooks`
2. Check that `.git/hooks/pre-commit` exists and is executable
3. Verify Deno is in your PATH

### Permission Issues
If you get permission errors:
```bash
chmod +x .git/hooks/pre-commit
chmod +x scripts/pre-commit.ts
chmod +x scripts/lint-staged.ts
chmod +x scripts/setup-hooks.ts
```

### Lint-Staged Issues
If lint-staged isn't working properly:
```bash
# Test manually
deno task lint-staged

# Check staged files
git diff --cached --name-status

# Reset and try again
git reset HEAD
git add <files>
```

## Performance Comparison

| Approach | Files Processed | Time (typical) |
|----------|----------------|----------------|
| Full codebase | ~70 files | 3-5 seconds |
| Lint-staged | 1-5 files | 0.5-1 second |

The lint-staged approach provides significant performance improvements, especially as the codebase grows.