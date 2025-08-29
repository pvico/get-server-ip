const std = @import("std");

pub fn out(comptime text: []const u8, args: anytype) !void {
    var stdout_buffer: [1024]u8 = undefined;
    var stdout_writer = std.fs.File.stdout().writer(&stdout_buffer);
    const stdout = &stdout_writer.interface;

    try stdout.print(text, args);
    try stdout.flush();
}

pub fn err(comptime text: []const u8, args: anytype) !void {
    var stderr_buffer: [1024]u8 = undefined;
    var stderr_writer = std.fs.File.stderr().writer(&stderr_buffer);
    const stderr = &stderr_writer.interface;

    try stderr.print(text, args);
    try stderr.flush();
}

// A function to add a new line to a string
pub fn outln(comptime text: []const u8) !void {
    try out("{s}\n", .{text});
}

pub fn errln(comptime text: []const u8) !void {
    try err("{s}\n", .{text});
}