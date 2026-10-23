/* Linux resource probe. A small exec'd launcher forks the measured compiler so
 * wait4's high-water RSS does not inherit the Python benchmark launcher's RSS.
 * Build outside timing: cc -O2 -o measure-child measure_child.c
 */
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 2) return 125;
    struct timespec before, after;
    if (clock_gettime(CLOCK_MONOTONIC, &before)) return 125;
    pid_t child = fork();
    if (child < 0) return 125;
    if (child == 0) {
        execvp(argv[1], argv + 1);
        _exit(126);
    }
    int status;
    struct rusage usage;
    while (wait4(child, &status, 0, &usage) < 0) {
        if (errno != EINTR) return 125;
    }
    if (clock_gettime(CLOCK_MONOTONIC, &after)) return 125;
    double wall = (after.tv_sec - before.tv_sec)
        + (after.tv_nsec - before.tv_nsec) / 1000000000.0;
    double cpu = usage.ru_utime.tv_sec + usage.ru_utime.tv_usec / 1000000.0
        + usage.ru_stime.tv_sec + usage.ru_stime.tv_usec / 1000000.0;
    fprintf(stderr, "nagi-benchmark-rusage: {\"wall_s\":%.9f,\"cpu_s\":%.9f,\"peak_child_rss_kib\":%ld}\n",
            wall, cpu, usage.ru_maxrss);
    if (WIFEXITED(status)) return WEXITSTATUS(status);
    return WIFSIGNALED(status) ? 128 + WTERMSIG(status) : 125;
}
