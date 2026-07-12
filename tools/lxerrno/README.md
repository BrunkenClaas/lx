# lxerrno

Translate error codes into plain English.

Explains HTTP status codes, POSIX errno codes, and shell exit codes. Well-known
codes are resolved locally (no network). Unknown codes fall back to the LLM.

## Usage

```
lxerrno 404
lxerrno ENOENT
lxerrno "exit 130"
lxerrno "errno 28"
echo "503" | lxerrno
```

## Output schema

```json
{
  "code":    "HTTP 404",
  "meaning": "Not Found — the requested resource could not be found on the server.",
  "hint":    "Verify the URL path, check for typos, and confirm the resource exists."
}
```

## Security flags

None

## Local lookup coverage

- **HTTP**: 100, 101, 200, 201, 204, 301, 302, 304, 307, 308, 400, 401, 403, 404, 405, 408, 409, 410, 422, 429, 500, 502, 503, 504
- **POSIX errno**: EPERM, ENOENT, ESRCH, EINTR, EIO, ENOEXEC, EBADF, ECHILD, EAGAIN, ENOMEM, EACCES, EFAULT, EBUSY, EEXIST, ENODEV, ENOTDIR, EISDIR, EINVAL, ENFILE, EMFILE, ENOSPC, EPIPE, ERANGE, EDEADLK, ENAMETOOLONG, ENOSYS, ENOTEMPTY, EOVERFLOW, ETIMEDOUT, ECONNREFUSED, EHOSTUNREACH
- **Exit codes**: 0, 1, 2, 126, 127, 128, 130, 137, 143, and any 128+N (signal N)

All other codes are explained by the configured LLM.
