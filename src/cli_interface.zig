const clap = @import("clap");

// // Define CLI options
// const params = comptime clap.parseParamsComptime(
//     \\-h, --help        Display this help and exit.
//     \\--gist            Display the IP from the Github gist
//     \\--h <ANSWER>
//     \\-i <ANSWER>...
//     \\<FILE>
// );

// const YesNo = enum { yes, no };
// const parsers = comptime .{
//     .STR = clap.parsers.string,
//     .FILE = clap.parsers.string,
//     .INT = clap.parsers.int(usize, 10),
//     .ANSWER = clap.parsers.enumeration(YesNo),
// };

// var diag = clap.Diagnostic{};
// var res = clap.parse(clap.Help, &params, parsers, .{
//     .diagnostic = &diag,
//     .allocator = allocator,
// }) catch |err| {
//     try diag.reportToFile(.stderr(), err);
//     return err;
// };
// defer res.deinit();

// if (res.args.help != 0) {
//     return clap.helpToFile(.stdout(), clap.Help, &params, .{});
// }
