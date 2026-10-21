using System;
using System.IO;
using System.Text.Json.Nodes;

internal static class Program
{
    private static void Main(string[] args)
    {
        if (args.Length != 2)
        {
            Console.Error.WriteLine("usage: Oracle <input-vectors.json> <output-vectors.json>");
            Environment.Exit(2);
        }

        var root = JsonNode.Parse(File.ReadAllText(args[0])).AsObject();

        foreach (var node in root["judge"].AsArray())
        {
            var item = node.AsObject();
            var kind = (NoteJudge.EJudgeType)item["type"].GetValue<int>();
            float diff = item["diff"].GetValue<float>();
            float option = item["option"].GetValue<float>();
            var timing = NoteJudge.GetJudgeTiming(ref diff, option, kind);
            item["timing"] = timing.ToString();
            item["after"] = diff;
        }

        foreach (var node in root["slide"].AsArray())
        {
            var item = node.AsObject();
            float diff = item["diff"].GetValue<float>();
            float wait = item["wait"].GetValue<float>();
            var timing = NoteJudge.GetSlideJudgeTiming(
                ref diff,
                0f,
                NoteJudge.EJudgeType.SlideOut,
                wait
            );
            item["timing"] = timing.ToString();
            item["after"] = diff;
        }

        foreach (var node in root["hold"].AsArray())
        {
            var item = node.AsObject();
            var timing = Enum.Parse<NoteJudge.ETiming>(item["timing"].GetValue<string>());
            var result = NoteJudge.JudgeHoldTotal(
                item["holdTime"].GetValue<float>(),
                item["release"].GetValue<float>(),
                timing,
                item["bodyOn"].GetValue<bool>(),
                item["touch"].GetValue<bool>()
            );
            item["result"] = result.ToString();
        }

        var scores = new JsonArray();
        for (int scoreType = 0; scoreType < 6; scoreType++)
        {
            var row = new JsonArray();
            for (int timing = 0; timing < 15; timing++)
            {
                row.Add(
                    NoteScore.GetJudgeScore(
                        (NoteJudge.ETiming)timing,
                        (NoteScore.EScoreType)scoreType
                    )
                );
            }
            scores.Add(row);
        }
        root["scores"] = scores;

        File.WriteAllText(args[1], root.ToJsonString());
        Console.WriteLine(args[1]);
    }
}
