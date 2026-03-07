window.BENCHMARK_DATA = {
  "lastUpdate": 1772924555631,
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
          "id": "7be791b5456931c2388b43b1b9b3eb9314ba4ed4",
          "message": "Fix test",
          "timestamp": "2026-03-07T03:44:27+01:00",
          "tree_id": "2eab9f45b4d6a3eb39ccfadcee8fd1a349bb0783",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/7be791b5456931c2388b43b1b9b3eb9314ba4ed4"
        },
        "date": 1772851773309,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28517,
            "range": "± 1770",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 129524,
            "range": "± 1198",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 236229,
            "range": "± 5116",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 474744,
            "range": "± 4202",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 528058,
            "range": "± 4009",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2586718,
            "range": "± 11930",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5225333,
            "range": "± 28304",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 132100,
            "range": "± 2807",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35724,
            "range": "± 203",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25805,
            "range": "± 608",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41066,
            "range": "± 295",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 140515,
            "range": "± 2750",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 662660,
            "range": "± 13038",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2603872,
            "range": "± 11117",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32430,
            "range": "± 726",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17459,
            "range": "± 127",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1494787,
            "range": "± 14324",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 65508,
            "range": "± 1853",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51873,
            "range": "± 170",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 91620433,
            "range": "± 1097703",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 505232,
            "range": "± 3744",
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
          "id": "6b613aac9971760f38e81be21ffab472d2791774",
          "message": "Fix Double-negated `instanceof` narrowing.",
          "timestamp": "2026-03-07T03:51:57+01:00",
          "tree_id": "ebeb49320cd88305da08cf31e4ab18c52f2b4eab",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/6b613aac9971760f38e81be21ffab472d2791774"
        },
        "date": 1772852230756,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28621,
            "range": "± 320",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 131263,
            "range": "± 3486",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 240068,
            "range": "± 15780",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 477681,
            "range": "± 32907",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 533542,
            "range": "± 7947",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2646732,
            "range": "± 1335249",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5342600,
            "range": "± 4172054",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 132693,
            "range": "± 13340",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36419,
            "range": "± 4952",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27479,
            "range": "± 1018",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42002,
            "range": "± 8113",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142059,
            "range": "± 15765",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 669516,
            "range": "± 87309",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2670105,
            "range": "± 1297375",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 31728,
            "range": "± 162",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17268,
            "range": "± 150",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1515746,
            "range": "± 14835",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 64118,
            "range": "± 595",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51443,
            "range": "± 222",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 93039238,
            "range": "± 1232932",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 522005,
            "range": "± 8794",
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
          "id": "3eff1f9bb6099c44912877b597f86224fef4966d",
          "message": "Accessor on new line with whitespace",
          "timestamp": "2026-03-07T03:59:21+01:00",
          "tree_id": "0f3ce99a782f200d3ec6c04c70051b4fbf8fe428",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/3eff1f9bb6099c44912877b597f86224fef4966d"
        },
        "date": 1772852671688,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28494,
            "range": "± 389",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 127753,
            "range": "± 1053",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 236684,
            "range": "± 1918",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 467830,
            "range": "± 4299",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 526025,
            "range": "± 7087",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2625903,
            "range": "± 21407",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5332347,
            "range": "± 36832",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 131909,
            "range": "± 791",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35061,
            "range": "± 3321",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26926,
            "range": "± 345",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 40682,
            "range": "± 292",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141888,
            "range": "± 1688",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 668823,
            "range": "± 3834",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2642163,
            "range": "± 29803",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 31617,
            "range": "± 935",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17577,
            "range": "± 208",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1498397,
            "range": "± 17062",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 64367,
            "range": "± 498",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51188,
            "range": "± 330",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 94098301,
            "range": "± 1714247",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 502772,
            "range": "± 14129",
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
          "id": "22a2b6a158010326503a8023525f4d6f1a47be71",
          "message": "Partial static property completion",
          "timestamp": "2026-03-07T04:09:15+01:00",
          "tree_id": "e5bbc703addd0127da982297c927f924d6917666",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/22a2b6a158010326503a8023525f4d6f1a47be71"
        },
        "date": 1772853262452,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28846,
            "range": "± 655",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 129562,
            "range": "± 1309",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 240718,
            "range": "± 2211",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 471138,
            "range": "± 3972",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 534192,
            "range": "± 12277",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2637464,
            "range": "± 101285",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5379747,
            "range": "± 62760",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 131513,
            "range": "± 6810",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35798,
            "range": "± 416",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26408,
            "range": "± 203",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41291,
            "range": "± 305",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 139723,
            "range": "± 2028",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 662385,
            "range": "± 25268",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2608707,
            "range": "± 10570",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32089,
            "range": "± 191",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17628,
            "range": "± 123",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1490935,
            "range": "± 37023",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 64995,
            "range": "± 438",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51334,
            "range": "± 652",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 92024708,
            "range": "± 1014932",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 506299,
            "range": "± 2078",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "6b20091e3f2072f554298d0608bddbf5984860a4",
          "message": "Update SETUP.md",
          "timestamp": "2026-03-07T04:12:08+01:00",
          "tree_id": "7fd1edf6ec8800727623ba7244592321eed7785a",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/6b20091e3f2072f554298d0608bddbf5984860a4"
        },
        "date": 1772853426031,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28070,
            "range": "± 549",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 127220,
            "range": "± 1466",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 236327,
            "range": "± 14360",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 470461,
            "range": "± 7655",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 515041,
            "range": "± 2622",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2572006,
            "range": "± 14668",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5094899,
            "range": "± 189881",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 128674,
            "range": "± 944",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35580,
            "range": "± 523",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25286,
            "range": "± 151",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 40773,
            "range": "± 1574",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 140562,
            "range": "± 1218",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 670282,
            "range": "± 3216",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2623313,
            "range": "± 53208",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32025,
            "range": "± 268",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17745,
            "range": "± 92",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1507788,
            "range": "± 23653",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 64570,
            "range": "± 573",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51509,
            "range": "± 209",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 92306844,
            "range": "± 1454192",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 508144,
            "range": "± 2352",
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
          "id": "2bf56f013f32d3736f1a778506d69858b4f31bea",
          "message": "Fix 3 bugs caught by PHPactor's tests",
          "timestamp": "2026-03-07T04:54:58+01:00",
          "tree_id": "843a259d5bad2f3e441049b46c6f5acc22ee547d",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/2bf56f013f32d3736f1a778506d69858b4f31bea"
        },
        "date": 1772856003791,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28860,
            "range": "± 370",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 127780,
            "range": "± 910",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 238502,
            "range": "± 3412",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 466621,
            "range": "± 2590",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 521267,
            "range": "± 4848",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2586990,
            "range": "± 16230",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5141256,
            "range": "± 42222",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 130402,
            "range": "± 1071",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35874,
            "range": "± 378",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26610,
            "range": "± 188",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41250,
            "range": "± 242",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 140741,
            "range": "± 582",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 663326,
            "range": "± 2411",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2624755,
            "range": "± 20558",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32613,
            "range": "± 263",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18867,
            "range": "± 81",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1499423,
            "range": "± 17669",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 64077,
            "range": "± 427",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51019,
            "range": "± 425",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 92953532,
            "range": "± 899310",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 527329,
            "range": "± 2572",
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
          "id": "319c4f530fa4a43a80770eac0eacd83c3a1cd688",
          "message": "Fix 4 bugs detected by PHPactor's tests",
          "timestamp": "2026-03-07T05:22:55+01:00",
          "tree_id": "3bf962bb5896ef8fef1552e0f513a4242a0e9eeb",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/319c4f530fa4a43a80770eac0eacd83c3a1cd688"
        },
        "date": 1772857682664,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29365,
            "range": "± 364",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 130531,
            "range": "± 2056",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 243366,
            "range": "± 3746",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 475493,
            "range": "± 2711",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 529911,
            "range": "± 10785",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2616106,
            "range": "± 26244",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5343763,
            "range": "± 42480",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 134470,
            "range": "± 1201",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35874,
            "range": "± 276",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26358,
            "range": "± 221",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41823,
            "range": "± 575",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141694,
            "range": "± 1989",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 670223,
            "range": "± 3460",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2627295,
            "range": "± 22833",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33423,
            "range": "± 293",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18765,
            "range": "± 231",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1525062,
            "range": "± 21360",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 64563,
            "range": "± 380",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51237,
            "range": "± 314",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 95536719,
            "range": "± 1552311",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 530399,
            "range": "± 17525",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "dan.t.leech@gmail.com",
            "name": "dantleech",
            "username": "dantleech"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9bd0a6f9de98e8908178ce1f3d8c057dcc4bf42e",
          "message": "Correct PHPactor status",
          "timestamp": "2026-03-07T15:56:44+01:00",
          "tree_id": "86e950cc3efb498fcf4c734835e591c0a5be5d10",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/9bd0a6f9de98e8908178ce1f3d8c057dcc4bf42e"
        },
        "date": 1772895705161,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28744,
            "range": "± 453",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 128636,
            "range": "± 2155",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 238748,
            "range": "± 2125",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 473504,
            "range": "± 3763",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 524326,
            "range": "± 10554",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2568896,
            "range": "± 19498",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5299223,
            "range": "± 93532",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 134529,
            "range": "± 2474",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35457,
            "range": "± 790",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26047,
            "range": "± 231",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 40974,
            "range": "± 734",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141705,
            "range": "± 3414",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 666483,
            "range": "± 3732",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2642740,
            "range": "± 250978",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32266,
            "range": "± 837",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18076,
            "range": "± 254",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1510575,
            "range": "± 16758",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 63492,
            "range": "± 806",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51239,
            "range": "± 458",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 91993214,
            "range": "± 528278",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 524321,
            "range": "± 27816",
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
          "id": "1108dbe30838047d271d845de128a7c2073dcff9",
          "message": "`@phpstan-assert` on static methods & Variadic `@param` template bindings",
          "timestamp": "2026-03-07T16:05:47+01:00",
          "tree_id": "5147a2f7905f5d51db1406bb91a290930ce8f4e6",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/1108dbe30838047d271d845de128a7c2073dcff9"
        },
        "date": 1772896253686,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29956,
            "range": "± 359",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 131796,
            "range": "± 1309",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 245374,
            "range": "± 2257",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 477653,
            "range": "± 3290",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 521464,
            "range": "± 5568",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2558195,
            "range": "± 15049",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5237583,
            "range": "± 41516",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 134180,
            "range": "± 1830",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36251,
            "range": "± 792",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25524,
            "range": "± 129",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42649,
            "range": "± 213",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141647,
            "range": "± 715",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 671003,
            "range": "± 6078",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2640612,
            "range": "± 23647",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32489,
            "range": "± 176",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19024,
            "range": "± 312",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1510066,
            "range": "± 33625",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70905,
            "range": "± 640",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50219,
            "range": "± 452",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101275082,
            "range": "± 432416",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 515402,
            "range": "± 3146",
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
          "id": "14f02e59c621531edabac01c584e9ad1f522dedc",
          "message": "Fix 5 type narrowing cases thanks to PHPactor's tests",
          "timestamp": "2026-03-07T16:59:29+01:00",
          "tree_id": "808d840d1e5b7738b0ce922aa9f787af78bc30ce",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/14f02e59c621531edabac01c584e9ad1f522dedc"
        },
        "date": 1772899482454,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 26124,
            "range": "± 426",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 114075,
            "range": "± 1738",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 220672,
            "range": "± 1194",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 441441,
            "range": "± 2503",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 481085,
            "range": "± 5565",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2649949,
            "range": "± 20976",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5642472,
            "range": "± 82190",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 123037,
            "range": "± 781",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 34539,
            "range": "± 269",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 24563,
            "range": "± 195",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 38824,
            "range": "± 251",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 133486,
            "range": "± 429",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 637953,
            "range": "± 4189",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2641372,
            "range": "± 13301",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 29417,
            "range": "± 194",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17142,
            "range": "± 80",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1462047,
            "range": "± 18308",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 74304,
            "range": "± 366",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 52539,
            "range": "± 272",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 97052480,
            "range": "± 577587",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 447801,
            "range": "± 3886",
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
          "id": "db767992d18d876d29398ddcdea80ba8740e11c8",
          "message": "Fix 4 cases of incorrect type narrowing thanks to PHPactor's tests",
          "timestamp": "2026-03-07T17:28:28+01:00",
          "tree_id": "de7a2a63f98778542c0d830b3654c38499f9a725",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/db767992d18d876d29398ddcdea80ba8740e11c8"
        },
        "date": 1772901217124,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29644,
            "range": "± 646",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 131090,
            "range": "± 1094",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 241857,
            "range": "± 1864",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 472871,
            "range": "± 3422",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 524331,
            "range": "± 3370",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2618575,
            "range": "± 29078",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5214522,
            "range": "± 36162",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 133029,
            "range": "± 797",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36940,
            "range": "± 211",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26830,
            "range": "± 578",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42628,
            "range": "± 432",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 145246,
            "range": "± 957",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 680043,
            "range": "± 3124",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2644341,
            "range": "± 68331",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33556,
            "range": "± 156",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18755,
            "range": "± 158",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1518171,
            "range": "± 14663",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70770,
            "range": "± 333",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51865,
            "range": "± 212",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 103251440,
            "range": "± 767982",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 520247,
            "range": "± 5013",
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
          "id": "98a1d78d49cc2c75926d531bc5828d4af33b485a",
          "message": "Workaround for PHP bug #7873",
          "timestamp": "2026-03-07T18:22:19+01:00",
          "tree_id": "8643e294e2cf018e81f4b6a653bd7e4dbfb86b10",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/98a1d78d49cc2c75926d531bc5828d4af33b485a"
        },
        "date": 1772904492743,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29090,
            "range": "± 439",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 131213,
            "range": "± 3050",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 243646,
            "range": "± 4444",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 476309,
            "range": "± 3533",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 532654,
            "range": "± 17083",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2618548,
            "range": "± 13039",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5322220,
            "range": "± 35968",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 133725,
            "range": "± 2900",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36841,
            "range": "± 325",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26157,
            "range": "± 369",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 43034,
            "range": "± 575",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142351,
            "range": "± 744",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 679530,
            "range": "± 9262",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2673732,
            "range": "± 14029",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33117,
            "range": "± 812",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18916,
            "range": "± 119",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1524606,
            "range": "± 35646",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70719,
            "range": "± 480",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 52137,
            "range": "± 657",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100846486,
            "range": "± 1892148",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 521717,
            "range": "± 4886",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0cd4c4eb916f056cdbfcc65f7bf65e7266556551",
          "message": "Update README.md",
          "timestamp": "2026-03-07T20:28:01+01:00",
          "tree_id": "c72bb81ab9bc046c55f3d31844c2ec15b9eefcec",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/0cd4c4eb916f056cdbfcc65f7bf65e7266556551"
        },
        "date": 1772911985424,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29458,
            "range": "± 546",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 129801,
            "range": "± 2207",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 241716,
            "range": "± 4173",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 473717,
            "range": "± 3043",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 526448,
            "range": "± 3328",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2598559,
            "range": "± 22062",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5264855,
            "range": "± 42474",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 134365,
            "range": "± 1617",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36584,
            "range": "± 607",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26873,
            "range": "± 4939",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42034,
            "range": "± 213",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142649,
            "range": "± 598",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 673395,
            "range": "± 7789",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2638836,
            "range": "± 20916",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33801,
            "range": "± 172",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18590,
            "range": "± 360",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1511712,
            "range": "± 29977",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70792,
            "range": "± 809",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51352,
            "range": "± 265",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100863759,
            "range": "± 660744",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 523120,
            "range": "± 3540",
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
          "id": "20afb5d82a55a28d918fc4f1d2bea6b8207478ac",
          "message": "Add assertions to demo to make sure we are _actually_ doing things right",
          "timestamp": "2026-03-07T20:24:25+01:00",
          "tree_id": "c9ad5e7508124b40d3c15ec8b338756fafe1c667",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/20afb5d82a55a28d918fc4f1d2bea6b8207478ac"
        },
        "date": 1772912074824,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29799,
            "range": "± 3999",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 132688,
            "range": "± 1156",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 245136,
            "range": "± 17408",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 476037,
            "range": "± 4900",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 532066,
            "range": "± 3242",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2653240,
            "range": "± 20873",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5165632,
            "range": "± 25496",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 134507,
            "range": "± 3040",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 37988,
            "range": "± 1670",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27065,
            "range": "± 310",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42480,
            "range": "± 197",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 143482,
            "range": "± 1168",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 678032,
            "range": "± 4639",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2653451,
            "range": "± 22481",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33117,
            "range": "± 290",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18942,
            "range": "± 133",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1529545,
            "range": "± 22410",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70855,
            "range": "± 613",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51188,
            "range": "± 272",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101550295,
            "range": "± 319915",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 526829,
            "range": "± 2659",
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
          "id": "a3c379d1a0c0f5956715f5429cb3eba027eb437a",
          "message": "Improve Generator Yield Type Inference Inside Bodies demo",
          "timestamp": "2026-03-07T20:43:17+01:00",
          "tree_id": "db2850a08c5f879aa11cc11ed6fe0831e9c6aa0d",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a3c379d1a0c0f5956715f5429cb3eba027eb437a"
        },
        "date": 1772912957146,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29416,
            "range": "± 302",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 127704,
            "range": "± 1045",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 244666,
            "range": "± 6276",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 475024,
            "range": "± 3180",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 524962,
            "range": "± 2201",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2612379,
            "range": "± 20514",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5294178,
            "range": "± 130087",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 136476,
            "range": "± 908",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36863,
            "range": "± 876",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27258,
            "range": "± 560",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42689,
            "range": "± 600",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 143289,
            "range": "± 3904",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 675079,
            "range": "± 3027",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2641596,
            "range": "± 16856",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 34740,
            "range": "± 956",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19134,
            "range": "± 1736",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1514710,
            "range": "± 25584",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70089,
            "range": "± 361",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51153,
            "range": "± 188",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101021565,
            "range": "± 2385358",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 529943,
            "range": "± 4579",
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
          "id": "ad8d5f9fe9f583e91162580ac381d678af73948b",
          "message": "Handle remaning issues fround by the PHPactor tests",
          "timestamp": "2026-03-07T22:00:21+01:00",
          "tree_id": "eb2587eda7ed21fba4217153df839407a7a6e1a1",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/ad8d5f9fe9f583e91162580ac381d678af73948b"
        },
        "date": 1772917533776,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28826,
            "range": "± 312",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 130355,
            "range": "± 1515",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 243608,
            "range": "± 4242",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 476972,
            "range": "± 4270",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 530720,
            "range": "± 5913",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2635680,
            "range": "± 96801",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5243051,
            "range": "± 29750",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 135904,
            "range": "± 1315",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36888,
            "range": "± 560",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25453,
            "range": "± 305",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41189,
            "range": "± 207",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142045,
            "range": "± 563",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 670831,
            "range": "± 6984",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2652076,
            "range": "± 27972",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32243,
            "range": "± 308",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17907,
            "range": "± 686",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1516162,
            "range": "± 27695",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70001,
            "range": "± 1053",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50805,
            "range": "± 236",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100243630,
            "range": "± 777589",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 517052,
            "range": "± 12790",
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
          "id": "b53ed2bd5fe04099b3e4878096efb91da9c0a65d",
          "message": "Make Generator Yield Type Inference Inside Bodies more real",
          "timestamp": "2026-03-07T22:08:10+01:00",
          "tree_id": "96adfd117c77b363cbc8c02078d9b1951f4f4024",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/b53ed2bd5fe04099b3e4878096efb91da9c0a65d"
        },
        "date": 1772918002209,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 26429,
            "range": "± 359",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 115926,
            "range": "± 3787",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 220834,
            "range": "± 924",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 442110,
            "range": "± 4116",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 480230,
            "range": "± 4025",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2689749,
            "range": "± 19749",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5719757,
            "range": "± 63059",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 122761,
            "range": "± 1001",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 34755,
            "range": "± 237",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26026,
            "range": "± 135",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 39259,
            "range": "± 1429",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 134783,
            "range": "± 1875",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 641987,
            "range": "± 13641",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2622808,
            "range": "± 54102",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 29301,
            "range": "± 159",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17470,
            "range": "± 68",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1458696,
            "range": "± 7899",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 74717,
            "range": "± 297",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 52494,
            "range": "± 606",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 95313271,
            "range": "± 789536",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 444576,
            "range": "± 1563",
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
          "id": "cee1e1b13db53d5b19fb6c625398e2ceaa6c625e",
          "message": "Make Generator Yield Type Inference Inside Bodies more real",
          "timestamp": "2026-03-07T22:16:23+01:00",
          "tree_id": "0c61b01daaa4bb5a91042360c1ddd60722b6e3fd",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/cee1e1b13db53d5b19fb6c625398e2ceaa6c625e"
        },
        "date": 1772918499158,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29141,
            "range": "± 680",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 127614,
            "range": "± 2583",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 241599,
            "range": "± 4835",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 475068,
            "range": "± 6557",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 529520,
            "range": "± 4712",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2605973,
            "range": "± 37063",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5282379,
            "range": "± 64710",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 134178,
            "range": "± 1805",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36311,
            "range": "± 326",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26722,
            "range": "± 574",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42196,
            "range": "± 871",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142148,
            "range": "± 1197",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 674194,
            "range": "± 16900",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2643089,
            "range": "± 50052",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33454,
            "range": "± 1017",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18246,
            "range": "± 195",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1517807,
            "range": "± 40260",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 69836,
            "range": "± 792",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50564,
            "range": "± 820",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 99538507,
            "range": "± 1760366",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 507258,
            "range": "± 7783",
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
          "id": "c1fc03fb950cf6cb75231463b92183c4d6418bdc",
          "message": "Prevent memory growth from file scanning",
          "timestamp": "2026-03-07T23:20:58+01:00",
          "tree_id": "6adc0354e916fc9f9f06eaf753a58989be945f62",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/c1fc03fb950cf6cb75231463b92183c4d6418bdc"
        },
        "date": 1772922844425,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 26247,
            "range": "± 330",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 123928,
            "range": "± 1297",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 226001,
            "range": "± 1173",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 444980,
            "range": "± 3377",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 481177,
            "range": "± 2669",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2690924,
            "range": "± 52410",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5801483,
            "range": "± 137061",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 123462,
            "range": "± 852",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 34723,
            "range": "± 183",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26377,
            "range": "± 209",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 39446,
            "range": "± 401",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 134609,
            "range": "± 711",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 641183,
            "range": "± 2215",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2641759,
            "range": "± 25945",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 28829,
            "range": "± 210",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17552,
            "range": "± 74",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1470487,
            "range": "± 45837",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 74312,
            "range": "± 2415",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51916,
            "range": "± 245",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 95395929,
            "range": "± 876706",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 446748,
            "range": "± 3501",
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
          "id": "3c5ee2915e08a3273ddaadb7e5fe2207684c5334",
          "message": "`instanceof` narrowing no longer widens specific types.",
          "timestamp": "2026-03-07T23:56:55+01:00",
          "tree_id": "dba80a42c7304cf34dc1670ec6f8802b863f7a0b",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/3c5ee2915e08a3273ddaadb7e5fe2207684c5334"
        },
        "date": 1772924555132,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28728,
            "range": "± 665",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 126019,
            "range": "± 859",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 237220,
            "range": "± 5283",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 477625,
            "range": "± 7974",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 545741,
            "range": "± 6516",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2706338,
            "range": "± 18827",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5574674,
            "range": "± 77980",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 133757,
            "range": "± 1727",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 35834,
            "range": "± 1303",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25724,
            "range": "± 174",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41295,
            "range": "± 332",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142124,
            "range": "± 1063",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 674016,
            "range": "± 6308",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2630427,
            "range": "± 12216",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33141,
            "range": "± 512",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18546,
            "range": "± 243",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1527026,
            "range": "± 26933",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70297,
            "range": "± 735",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50218,
            "range": "± 299",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100769532,
            "range": "± 970640",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 512428,
            "range": "± 6737",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}