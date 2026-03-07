window.BENCHMARK_DATA = {
  "lastUpdate": 1772843023295,
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
      },
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
          "id": "23f8059894ae93679c2144940735741407228e22",
          "message": "Add diagnostics test from PHPactor",
          "timestamp": "2026-03-05T13:52:40+01:00",
          "tree_id": "39abde505c1a1183b8681d94fe51f08e148463b1",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/23f8059894ae93679c2144940735741407228e22"
        },
        "date": 1772715440275,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 27739,
            "range": "± 410",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 125307,
            "range": "± 1377",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 234896,
            "range": "± 4390",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 457968,
            "range": "± 2355",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 531962,
            "range": "± 11541",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2561497,
            "range": "± 16560",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5109967,
            "range": "± 51536",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 127144,
            "range": "± 1804",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35069,
            "range": "± 482",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26363,
            "range": "± 494",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 40197,
            "range": "± 810",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 139521,
            "range": "± 1571",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 662600,
            "range": "± 3229",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2602743,
            "range": "± 34875",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 34256,
            "range": "± 370",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17245,
            "range": "± 1594",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1330276,
            "range": "± 14708",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 52532,
            "range": "± 332",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 39290,
            "range": "± 378",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "bb60b6b67cd8f15208465dfae3fc1740e9ef1936",
          "message": "Acknowledge Phpactor's test suite and benchmark fixtures in README",
          "timestamp": "2026-03-05T15:16:21+01:00",
          "tree_id": "6de2f7c6d33a869eb2a1397ba6959cb98bf369a0",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/bb60b6b67cd8f15208465dfae3fc1740e9ef1936"
        },
        "date": 1772720464603,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 27885,
            "range": "± 423",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 123030,
            "range": "± 2273",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 235643,
            "range": "± 2322",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 463083,
            "range": "± 8396",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 528210,
            "range": "± 15002",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2574513,
            "range": "± 26243",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5158834,
            "range": "± 195643",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 129203,
            "range": "± 8448",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35221,
            "range": "± 530",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26795,
            "range": "± 1616",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 40623,
            "range": "± 1534",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141217,
            "range": "± 2060",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 671968,
            "range": "± 4700",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2623501,
            "range": "± 22181",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35241,
            "range": "± 170",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 16840,
            "range": "± 122",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1339786,
            "range": "± 54572",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 52741,
            "range": "± 501",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 39325,
            "range": "± 1534",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "cdwhite3@pm.me",
            "name": "Caleb White",
            "username": "calebdw"
          },
          "committer": {
            "email": "anders@jenbo.dk",
            "name": "Anders Jenbo",
            "username": "AJenbo"
          },
          "distinct": true,
          "id": "d8faf747a5817449a7db816f59b1c18d7faf1662",
          "message": "feat: add --version and --help support",
          "timestamp": "2026-03-05T16:14:51+01:00",
          "tree_id": "0635806d32ce2027a32a9ca55a1c1565afbc9ac6",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/d8faf747a5817449a7db816f59b1c18d7faf1662"
        },
        "date": 1772723981125,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 27798,
            "range": "± 504",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 123433,
            "range": "± 5386",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 235423,
            "range": "± 1382",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 461869,
            "range": "± 12610",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 522790,
            "range": "± 5065",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2600104,
            "range": "± 41296",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5152075,
            "range": "± 169226",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 127261,
            "range": "± 1409",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 34327,
            "range": "± 402",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25813,
            "range": "± 239",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 39474,
            "range": "± 341",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 140774,
            "range": "± 2092",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 666889,
            "range": "± 12420",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2598752,
            "range": "± 21924",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35260,
            "range": "± 239",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 16675,
            "range": "± 82",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1328439,
            "range": "± 24278",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 51558,
            "range": "± 316",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 39469,
            "range": "± 183",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "4bc59e2e7d60a12daf800da6fc0424fbdc0b287b",
          "message": "Fix performance regression from diagnostics",
          "timestamp": "2026-03-05T17:13:55+01:00",
          "tree_id": "afc9c3bfa91489247cff9dd9e25783fbc3334a5e",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/4bc59e2e7d60a12daf800da6fc0424fbdc0b287b"
        },
        "date": 1772727501944,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 24398,
            "range": "± 616",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 115041,
            "range": "± 3727",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 226045,
            "range": "± 1381",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 457733,
            "range": "± 4718",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 535691,
            "range": "± 5447",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2741193,
            "range": "± 23001",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5664774,
            "range": "± 66907",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 112533,
            "range": "± 3464",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 33036,
            "range": "± 652",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 24167,
            "range": "± 521",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 35871,
            "range": "± 483",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142099,
            "range": "± 632",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 680106,
            "range": "± 23875",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2742746,
            "range": "± 21629",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 28882,
            "range": "± 201",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 15398,
            "range": "± 196",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1365612,
            "range": "± 83324",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 51519,
            "range": "± 691",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 40537,
            "range": "± 152",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "ff8f6375a56a4b8287401007e8de37480f7747bb",
          "message": "Fix overly agressive caching leading to incorrect type tracking.",
          "timestamp": "2026-03-05T18:50:28+01:00",
          "tree_id": "b005f38941ef35eddbe4460d0a7f146e9c3937ca",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/ff8f6375a56a4b8287401007e8de37480f7747bb"
        },
        "date": 1772734336898,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28114,
            "range": "± 1773",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 124481,
            "range": "± 2200",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 235750,
            "range": "± 6982",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 460675,
            "range": "± 6443",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 512285,
            "range": "± 12678",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2560435,
            "range": "± 57818",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5246469,
            "range": "± 361907",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 128103,
            "range": "± 1375",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35597,
            "range": "± 691",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25848,
            "range": "± 235",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 40132,
            "range": "± 281",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 140395,
            "range": "± 653",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 663708,
            "range": "± 5887",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2599598,
            "range": "± 23913",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32278,
            "range": "± 253",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18044,
            "range": "± 278",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1341540,
            "range": "± 14683",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 51680,
            "range": "± 762",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 39852,
            "range": "± 276",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "2a6b81f807e59c5680f699c47a20618824b4b316",
          "message": "Improve caching and workout a better cache strategy for the future",
          "timestamp": "2026-03-05T19:43:21+01:00",
          "tree_id": "0c908b8f44c92c7b504bb25f83e4ab967fae54bf",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/2a6b81f807e59c5680f699c47a20618824b4b316"
        },
        "date": 1772736590638,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28449,
            "range": "± 326",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 123608,
            "range": "± 659",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 239859,
            "range": "± 2991",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 465883,
            "range": "± 3157",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 523726,
            "range": "± 3054",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2549928,
            "range": "± 11238",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5163002,
            "range": "± 182848",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 128129,
            "range": "± 4393",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35040,
            "range": "± 405",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25494,
            "range": "± 192",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 40332,
            "range": "± 1201",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 139772,
            "range": "± 520",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 665298,
            "range": "± 4055",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2604115,
            "range": "± 14199",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 31132,
            "range": "± 112",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17118,
            "range": "± 933",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1348396,
            "range": "± 13478",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 52246,
            "range": "± 456",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 39776,
            "range": "± 714",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "871d7eb797e73854a25921c347e2845fb7089c26",
          "message": "Fully implement Go To Implementation",
          "timestamp": "2026-03-05T21:08:30+01:00",
          "tree_id": "af854b90ebde70cba8f9917031ee57d9e81f2c42",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/871d7eb797e73854a25921c347e2845fb7089c26"
        },
        "date": 1772741673110,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28499,
            "range": "± 3095",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 128593,
            "range": "± 1230",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 239860,
            "range": "± 3575",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 472414,
            "range": "± 7076",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 535993,
            "range": "± 4012",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2592200,
            "range": "± 17364",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5269148,
            "range": "± 41538",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 131847,
            "range": "± 1047",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35914,
            "range": "± 170",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25894,
            "range": "± 186",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41513,
            "range": "± 339",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 140341,
            "range": "± 1640",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 668496,
            "range": "± 4970",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2619901,
            "range": "± 141195",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32136,
            "range": "± 538",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17231,
            "range": "± 146",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1353338,
            "range": "± 13940",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 51776,
            "range": "± 267",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 38969,
            "range": "± 283",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "0110a172de32a1ed871dc41d221e3e0cbfc5f74b",
          "message": "Implement unknown member diagnostics.",
          "timestamp": "2026-03-05T22:09:14+01:00",
          "tree_id": "2b2f89fcfa9d5a084f6000d6f5170d089a707aa4",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/0110a172de32a1ed871dc41d221e3e0cbfc5f74b"
        },
        "date": 1772745269555,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 24409,
            "range": "± 740",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 114882,
            "range": "± 3378",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 231709,
            "range": "± 3027",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 462762,
            "range": "± 20637",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 534549,
            "range": "± 5174",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2707756,
            "range": "± 20191",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5749980,
            "range": "± 245990",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 111522,
            "range": "± 1225",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 33425,
            "range": "± 199",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25396,
            "range": "± 291",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 35394,
            "range": "± 325",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 140786,
            "range": "± 1911",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 675551,
            "range": "± 28418",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2681818,
            "range": "± 9030",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 25421,
            "range": "± 128",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 15444,
            "range": "± 664",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1375374,
            "range": "± 18865",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 63331,
            "range": "± 1114",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 52342,
            "range": "± 968",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 92865459,
            "range": "± 4547210",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 543210,
            "range": "± 4411",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "41b89da9bb84696b0545b28a6bd11d4e46dc9836",
          "message": "Implement Rename Symbol",
          "timestamp": "2026-03-05T22:18:09+01:00",
          "tree_id": "6e00558f1a67a9a0576066f0a321282329356d93",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/41b89da9bb84696b0545b28a6bd11d4e46dc9836"
        },
        "date": 1772745817720,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28283,
            "range": "± 475",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 129006,
            "range": "± 1465",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 243230,
            "range": "± 2419",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 477122,
            "range": "± 4460",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 533772,
            "range": "± 4031",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2600244,
            "range": "± 34844",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5173689,
            "range": "± 191869",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 130592,
            "range": "± 1515",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35992,
            "range": "± 352",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26050,
            "range": "± 212",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 40141,
            "range": "± 385",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141493,
            "range": "± 1290",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 668258,
            "range": "± 5714",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2624845,
            "range": "± 66408",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 30547,
            "range": "± 219",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17459,
            "range": "± 1157",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1350178,
            "range": "± 27052",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 62671,
            "range": "± 384",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50917,
            "range": "± 277",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 91650455,
            "range": "± 509237",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 521547,
            "range": "± 3087",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "93cc1a86253b4463d4bbc5012a3194ea226d7d6a",
          "message": "Add implement missing methods action",
          "timestamp": "2026-03-05T23:30:45+01:00",
          "tree_id": "d1310a9142b4807713360dffa229915a4c8fb592",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/93cc1a86253b4463d4bbc5012a3194ea226d7d6a"
        },
        "date": 1772750158032,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28483,
            "range": "± 550",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 127215,
            "range": "± 2602",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 239514,
            "range": "± 3522",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 472125,
            "range": "± 4565",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 524844,
            "range": "± 3672",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2590771,
            "range": "± 22886",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5287309,
            "range": "± 141898",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 128941,
            "range": "± 2083",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36336,
            "range": "± 337",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26915,
            "range": "± 514",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41447,
            "range": "± 280",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 139923,
            "range": "± 3763",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 660619,
            "range": "± 37433",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2586803,
            "range": "± 9006",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 31657,
            "range": "± 413",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18060,
            "range": "± 155",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1340240,
            "range": "± 27323",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 63546,
            "range": "± 913",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50513,
            "range": "± 269",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 92787664,
            "range": "± 1215444",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 519782,
            "range": "± 13301",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "b76c6392123a2c728ac77229ac12f04a95521bf3",
          "message": "Add a configand member-on-unknown diagnostics (off by default)",
          "timestamp": "2026-03-07T00:25:40+01:00",
          "tree_id": "4fb40fd9efad7c34be7cd06561b458ec9a453881",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/b76c6392123a2c728ac77229ac12f04a95521bf3"
        },
        "date": 1772839853228,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 24809,
            "range": "± 264",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 118242,
            "range": "± 1118",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 223821,
            "range": "± 1580",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 439927,
            "range": "± 2385",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 482196,
            "range": "± 2706",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2659268,
            "range": "± 29611",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5484709,
            "range": "± 108837",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 117049,
            "range": "± 1108",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 33579,
            "range": "± 176",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25564,
            "range": "± 262",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 36789,
            "range": "± 340",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 133520,
            "range": "± 994",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 639032,
            "range": "± 3282",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2647580,
            "range": "± 7938",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 27403,
            "range": "± 520",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 16039,
            "range": "± 59",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1325249,
            "range": "± 15512",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 64841,
            "range": "± 430",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 52815,
            "range": "± 720",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 86652627,
            "range": "± 384936",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 441085,
            "range": "± 1702",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "7bb5660d3fe7581ef442abef64a352769e792074",
          "message": "Improve caching",
          "timestamp": "2026-03-07T00:52:30+01:00",
          "tree_id": "2e0728d88abe5ec7be3d8fb21bb9a0b6dad9b756",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/7bb5660d3fe7581ef442abef64a352769e792074"
        },
        "date": 1772841459966,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28306,
            "range": "± 308",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 129392,
            "range": "± 1356",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 236642,
            "range": "± 4667",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 469907,
            "range": "± 6252",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 524839,
            "range": "± 4754",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2569440,
            "range": "± 22747",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5351396,
            "range": "± 75347",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 127854,
            "range": "± 2334",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35909,
            "range": "± 1150",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26641,
            "range": "± 1283",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41033,
            "range": "± 821",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141049,
            "range": "± 4728",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 665723,
            "range": "± 5905",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2630618,
            "range": "± 83547",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33037,
            "range": "± 1961",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17125,
            "range": "± 511",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1501982,
            "range": "± 28345",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 65026,
            "range": "± 325",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 52146,
            "range": "± 228",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 93347766,
            "range": "± 2637216",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 514665,
            "range": "± 2480",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "b1c5de0ecb0b40d91c02527bdbfefb2155e3210d",
          "message": "Fix tests",
          "timestamp": "2026-03-07T01:18:41+01:00",
          "tree_id": "d64cec08be814541514026e4f674ee9a9443db81",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/b1c5de0ecb0b40d91c02527bdbfefb2155e3210d"
        },
        "date": 1772843022865,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28030,
            "range": "± 529",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 127024,
            "range": "± 1653",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 237379,
            "range": "± 3553",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 468994,
            "range": "± 5942",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 519071,
            "range": "± 4666",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2576476,
            "range": "± 9859",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5230210,
            "range": "± 41855",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 128119,
            "range": "± 1051",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35261,
            "range": "± 180",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25742,
            "range": "± 174",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 40439,
            "range": "± 298",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141453,
            "range": "± 784",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 668132,
            "range": "± 3522",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2639067,
            "range": "± 100499",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 31555,
            "range": "± 237",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17484,
            "range": "± 141",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1512653,
            "range": "± 13749",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 66475,
            "range": "± 376",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 53246,
            "range": "± 275",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 93296776,
            "range": "± 1937272",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 507898,
            "range": "± 3359",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}