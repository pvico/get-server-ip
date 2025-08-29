const std = @import("std");
const print = @import("../utilities/print.zig");
const builtin = @import("builtin");

const DEBUG_MODE = builtin.mode == std.builtin.OptimizeMode.Debug;

pub fn info(comptime fmt: []const u8, args: anytype) void {
    const text: []const u8 = "\x1b[32mINFO " ++ fmt ++ "\x1b[0m\n";
    print.out(text, args) catch {};
}

pub fn debug(comptime fmt: []const u8, args: anytype) void {
    if (DEBUG_MODE) {
        const text: []const u8 = "\x1b[35mDEBUG " ++ fmt ++ "\x1b[0m\n";
        print.err(text, args) catch {};
    }
}

pub fn warn(comptime fmt: []const u8, args: anytype) void {
    const text: []const u8 = "\x1b[33mWARNING " ++ fmt ++ "\x1b[0m\n";
    print.err(text, args) catch {};
}

pub fn err(comptime fmt: []const u8, args: anytype) void {
    const text: []const u8 = "\x1b[91mERROR " ++ fmt ++ "\x1b[0m\n";
    print.err(text, args) catch {};
}

pub fn fatal(comptime fmt: []const u8, args: anytype) void {
    const text: []const u8 = "\x1b[97;41mFATAL " ++ fmt ++ "\x1b[0m\n";
    print.err(text, args) catch {};
}
