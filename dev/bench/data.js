window.BENCHMARK_DATA = {
  "lastUpdate": 1772706681694,
  "repoUrl": "https://github.com/AJenbo/phpantom_lsp",
  "entries": {
    "PHPantom Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "4de6add052e252d3187a30781244e2b30bc74362",
          "message": "Fix CI job",
          "timestamp": "2026-03-05T11:25:54+01:00",
          "tree_id": "20a52ef787c9695630e7c290eca044779a2545ff",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/4de6add052e252d3187a30781244e2b30bc74362"
        },
        "date": 1772706681002,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 25083,
            "range": "± 596",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 117334,
            "range": "± 1055",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 214435,
            "range": "± 2367",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 429860,
            "range": "± 3425",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 476785,
            "range": "± 3247",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2624643,
            "range": "± 12126",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5310762,
            "range": "± 33182",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 113892,
            "range": "± 965",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 32914,
            "range": "± 89",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 24606,
            "range": "± 140",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 36609,
            "range": "± 142",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 132484,
            "range": "± 791",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 628813,
            "range": "± 6533",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2566897,
            "range": "± 16155",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 30743,
            "range": "± 132",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 16004,
            "range": "± 192",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1297130,
            "range": "± 22201",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}