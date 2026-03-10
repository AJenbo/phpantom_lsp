window.BENCHMARK_DATA = {
  "lastUpdate": 1773163420595,
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
          "id": "5a5c7408aab98d2d62610cd854a92289b15f89f3",
          "message": "Variable resolution: static chain assignment",
          "timestamp": "2026-03-08T01:25:47+01:00",
          "tree_id": "2504bce8f50b23b1be433a9aef5e52783f298ec7",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/5a5c7408aab98d2d62610cd854a92289b15f89f3"
        },
        "date": 1772929858884,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28645,
            "range": "± 630",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 130357,
            "range": "± 1295",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 244866,
            "range": "± 6593",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 476256,
            "range": "± 2598",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 531502,
            "range": "± 14629",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2608166,
            "range": "± 17630",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5296159,
            "range": "± 56011",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 132016,
            "range": "± 958",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36329,
            "range": "± 294",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26870,
            "range": "± 739",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41430,
            "range": "± 289",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 140818,
            "range": "± 847",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 665775,
            "range": "± 6395",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2614828,
            "range": "± 15065",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32953,
            "range": "± 150",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18291,
            "range": "± 89",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1519216,
            "range": "± 25377",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 71046,
            "range": "± 861",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50889,
            "range": "± 330",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100669834,
            "range": "± 2301775",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 516773,
            "range": "± 2291",
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
          "id": "5c2caab27a2ebf4102e980fff910265aa19dbb60",
          "message": "False-positive unknown-class warnings on PHPStan type syntax",
          "timestamp": "2026-03-08T01:48:09+01:00",
          "tree_id": "fc045de875ac24434f3c29e85113923e07beb959",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/5c2caab27a2ebf4102e980fff910265aa19dbb60"
        },
        "date": 1772931203299,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 30362,
            "range": "± 666",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 130733,
            "range": "± 3098",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 246808,
            "range": "± 2835",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 483678,
            "range": "± 3642",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 537179,
            "range": "± 6328",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2672511,
            "range": "± 34813",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5434180,
            "range": "± 126175",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 136822,
            "range": "± 1473",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36920,
            "range": "± 465",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25740,
            "range": "± 128",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42865,
            "range": "± 790",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142534,
            "range": "± 2798",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 673315,
            "range": "± 12434",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2639509,
            "range": "± 48424",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 34478,
            "range": "± 221",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19053,
            "range": "± 380",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1527418,
            "range": "± 17298",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 69911,
            "range": "± 526",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51176,
            "range": "± 491",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 102856247,
            "range": "± 1281220",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 522900,
            "range": "± 2925",
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
          "id": "4f285fbab2409cd0343477b1115758b4dca98fc4",
          "message": "Fix massive startup delay (waiting for full dianostics)",
          "timestamp": "2026-03-08T02:06:05+01:00",
          "tree_id": "569c8fadd84e95c477be2c494fa88b798fbb48d4",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/4f285fbab2409cd0343477b1115758b4dca98fc4"
        },
        "date": 1772932278994,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28986,
            "range": "± 341",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 130967,
            "range": "± 1850",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 244729,
            "range": "± 1693",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 475506,
            "range": "± 3678",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 532654,
            "range": "± 8434",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2660708,
            "range": "± 19226",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5197830,
            "range": "± 87159",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 138874,
            "range": "± 1036",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36767,
            "range": "± 309",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27205,
            "range": "± 248",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42519,
            "range": "± 269",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 145071,
            "range": "± 1496",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 682632,
            "range": "± 5406",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2659108,
            "range": "± 20460",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 34859,
            "range": "± 340",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18810,
            "range": "± 137",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1525659,
            "range": "± 29073",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70922,
            "range": "± 698",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50137,
            "range": "± 247",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 103401284,
            "range": "± 479079",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 543046,
            "range": "± 3189",
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
          "id": "513a0dd359cdb38821628613e19dd949399bb61e",
          "message": "Update perf numbers now that we have diagnostics",
          "timestamp": "2026-03-08T02:38:43+01:00",
          "tree_id": "475aaf2ca0daa6d537388d5605ba44a6dbf814e0",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/513a0dd359cdb38821628613e19dd949399bb61e"
        },
        "date": 1772934228965,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28635,
            "range": "± 979",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 125810,
            "range": "± 2000",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 242008,
            "range": "± 4130",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 473913,
            "range": "± 5814",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 528214,
            "range": "± 8638",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2589009,
            "range": "± 18152",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5224124,
            "range": "± 35893",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 135186,
            "range": "± 1007",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36415,
            "range": "± 1285",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26657,
            "range": "± 182",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42005,
            "range": "± 253",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141480,
            "range": "± 2930",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 674523,
            "range": "± 15589",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2637924,
            "range": "± 12220",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33889,
            "range": "± 4571",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18224,
            "range": "± 157",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1516461,
            "range": "± 37626",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 71333,
            "range": "± 269",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50668,
            "range": "± 160",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 104420713,
            "range": "± 2689012",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 530074,
            "range": "± 2446",
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
          "id": "558e2f45f0b3edd3319c627d17a669926faae9ff",
          "message": "Add indexing roadmap",
          "timestamp": "2026-03-08T05:49:26+01:00",
          "tree_id": "ca7207ff7ea958959e2208642832aaf24545e195",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/558e2f45f0b3edd3319c627d17a669926faae9ff"
        },
        "date": 1772945677331,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28531,
            "range": "± 587",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 126362,
            "range": "± 633",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 238113,
            "range": "± 1703",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 474893,
            "range": "± 3095",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 534567,
            "range": "± 4194",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2609969,
            "range": "± 16895",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5242604,
            "range": "± 106975",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 135113,
            "range": "± 2807",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36951,
            "range": "± 486",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27389,
            "range": "± 176",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41592,
            "range": "± 325",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 144311,
            "range": "± 907",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 689318,
            "range": "± 3845",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2717482,
            "range": "± 62601",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33777,
            "range": "± 258",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18874,
            "range": "± 106",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1553819,
            "range": "± 15101",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 71140,
            "range": "± 418",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50418,
            "range": "± 156",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 103529031,
            "range": "± 343035",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 528234,
            "range": "± 2022",
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
          "id": "d482e8aab050135064d7332c8bb6bb466c5cc2ac",
          "message": "Add performance to roadmap",
          "timestamp": "2026-03-08T06:25:32+01:00",
          "tree_id": "13e95625ca632cafa2eb0ad8b263d9c877a01081",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/d482e8aab050135064d7332c8bb6bb466c5cc2ac"
        },
        "date": 1772947839742,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28774,
            "range": "± 251",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 129529,
            "range": "± 7093",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 244186,
            "range": "± 2051",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 475828,
            "range": "± 3943",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 529070,
            "range": "± 9006",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2582980,
            "range": "± 8105",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5335340,
            "range": "± 95953",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 134636,
            "range": "± 968",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36855,
            "range": "± 383",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27016,
            "range": "± 185",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41515,
            "range": "± 1695",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142405,
            "range": "± 1313",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 673482,
            "range": "± 7003",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2649148,
            "range": "± 21298",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 34171,
            "range": "± 887",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19034,
            "range": "± 100",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1524624,
            "range": "± 12571",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 71162,
            "range": "± 628",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50807,
            "range": "± 1419",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101962296,
            "range": "± 2027709",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 522940,
            "range": "± 1903",
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
          "id": "93eb32868331c0f537aa8b197a19eee6378b57bf",
          "message": "Implement file scanner fallback for when composer is not optimized",
          "timestamp": "2026-03-08T15:00:38+01:00",
          "tree_id": "0103ad40b3491e05028d3bef43b802820585a8fc",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/93eb32868331c0f537aa8b197a19eee6378b57bf"
        },
        "date": 1772978753738,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29585,
            "range": "± 496",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 130834,
            "range": "± 1112",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 243877,
            "range": "± 5019",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 482432,
            "range": "± 12997",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 539539,
            "range": "± 4968",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2630039,
            "range": "± 17081",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5367551,
            "range": "± 42989",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 137706,
            "range": "± 3893",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36619,
            "range": "± 585",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26882,
            "range": "± 915",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42332,
            "range": "± 257",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 144527,
            "range": "± 960",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 688717,
            "range": "± 3980",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2719452,
            "range": "± 36547",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 34361,
            "range": "± 193",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18946,
            "range": "± 125",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1558011,
            "range": "± 16123",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 71369,
            "range": "± 748",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 52459,
            "range": "± 919",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 103586767,
            "range": "± 1402652",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 542848,
            "range": "± 8891",
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
          "id": "d49104063096d4a89003f2cb9991261ff68e5e52",
          "message": "Add plan for stub overrides",
          "timestamp": "2026-03-08T16:25:25+01:00",
          "tree_id": "002b293e5a554e69f40e9d845940e4d5cc3a4f72",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/d49104063096d4a89003f2cb9991261ff68e5e52"
        },
        "date": 1772983834264,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28803,
            "range": "± 946",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 128855,
            "range": "± 941",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 241556,
            "range": "± 2044",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 475088,
            "range": "± 3310",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 530790,
            "range": "± 3107",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2610936,
            "range": "± 36818",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5259869,
            "range": "± 25033",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 134332,
            "range": "± 865",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36298,
            "range": "± 263",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26868,
            "range": "± 982",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41268,
            "range": "± 511",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142612,
            "range": "± 691",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 675598,
            "range": "± 3076",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2648598,
            "range": "± 23514",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32848,
            "range": "± 778",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18311,
            "range": "± 158",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1511752,
            "range": "± 24288",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 69549,
            "range": "± 458",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51163,
            "range": "± 379",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100356801,
            "range": "± 1712461",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 524912,
            "range": "± 7007",
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
          "id": "373b68ec3cdb21807f65e26e30d7aefb62c1feb8",
          "message": "Add plan for stub overrides",
          "timestamp": "2026-03-08T16:37:03+01:00",
          "tree_id": "97ef0d614e36b026d1169ead730a003ebce3bb9e",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/373b68ec3cdb21807f65e26e30d7aefb62c1feb8"
        },
        "date": 1772984538891,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29417,
            "range": "± 462",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 132518,
            "range": "± 778",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 247141,
            "range": "± 3799",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 479902,
            "range": "± 6100",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 532918,
            "range": "± 10933",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2609633,
            "range": "± 39739",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5179315,
            "range": "± 56252",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 136421,
            "range": "± 1527",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36449,
            "range": "± 340",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26955,
            "range": "± 268",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 42539,
            "range": "± 242",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141806,
            "range": "± 1020",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 674069,
            "range": "± 3316",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2633971,
            "range": "± 11397",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33536,
            "range": "± 184",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19135,
            "range": "± 193",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1520532,
            "range": "± 30018",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 69903,
            "range": "± 664",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50930,
            "range": "± 457",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100291736,
            "range": "± 783210",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 526026,
            "range": "± 2367",
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
          "id": "6ca201ebc413c0798a2a7a299ca0019db5b2b466",
          "message": "Update roadmap",
          "timestamp": "2026-03-08T16:55:49+01:00",
          "tree_id": "3cd5111619c3c4ac7caea5050d413d63f735fdcc",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/6ca201ebc413c0798a2a7a299ca0019db5b2b466"
        },
        "date": 1772985655333,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28771,
            "range": "± 311",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 125539,
            "range": "± 923",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 243788,
            "range": "± 1918",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 473276,
            "range": "± 10127",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 544205,
            "range": "± 4738",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2649347,
            "range": "± 37652",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5273847,
            "range": "± 24663",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 135709,
            "range": "± 1031",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36657,
            "range": "± 266",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27109,
            "range": "± 1494",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 41724,
            "range": "± 335",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 141905,
            "range": "± 2228",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 674605,
            "range": "± 4553",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2656774,
            "range": "± 13581",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 32757,
            "range": "± 174",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18253,
            "range": "± 92",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1521974,
            "range": "± 12972",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70513,
            "range": "± 381",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 51370,
            "range": "± 288",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 99996649,
            "range": "± 661778",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 523503,
            "range": "± 2403",
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
          "id": "a7366399aade0a621f43760e82475e009993c7c1",
          "message": "Concurrent read access to shared state",
          "timestamp": "2026-03-08T18:03:51+01:00",
          "tree_id": "96e942c4f49a7c5352c3b8df2182591b3a057472",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a7366399aade0a621f43760e82475e009993c7c1"
        },
        "date": 1772989739314,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29408,
            "range": "± 245",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 130364,
            "range": "± 1915",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 240504,
            "range": "± 47403",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 477984,
            "range": "± 4676",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 527961,
            "range": "± 4889",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2620542,
            "range": "± 95253",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5308404,
            "range": "± 137307",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 135213,
            "range": "± 5267",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36683,
            "range": "± 303",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 26615,
            "range": "± 200",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 43025,
            "range": "± 298",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 142783,
            "range": "± 2228",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 678663,
            "range": "± 18213",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2638213,
            "range": "± 12231",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 33058,
            "range": "± 340",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 18594,
            "range": "± 391",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1536659,
            "range": "± 24556",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 70493,
            "range": "± 541",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 50614,
            "range": "± 570",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100345158,
            "range": "± 2108080",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 511410,
            "range": "± 4284",
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
          "id": "4da4dde05e5afcbe32b37bac296647cac01205c2",
          "message": "Parallel file processing",
          "timestamp": "2026-03-08T18:30:04+01:00",
          "tree_id": "0e44a1ad3ef1bf45755288623fcc0747418a15b0",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/4da4dde05e5afcbe32b37bac296647cac01205c2"
        },
        "date": 1772991313929,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 24951,
            "range": "± 447",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 113638,
            "range": "± 361",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 224876,
            "range": "± 4509",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 435158,
            "range": "± 1702",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 467881,
            "range": "± 3540",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2582191,
            "range": "± 18855",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5249979,
            "range": "± 42435",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 124054,
            "range": "± 3955",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 32867,
            "range": "± 134",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 24664,
            "range": "± 169",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 38280,
            "range": "± 131",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 133092,
            "range": "± 605",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 638216,
            "range": "± 2771",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2626813,
            "range": "± 9986",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 29105,
            "range": "± 137",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 17121,
            "range": "± 76",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1458562,
            "range": "± 39488",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 38561,
            "range": "± 292",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 16870,
            "range": "± 48",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 94715235,
            "range": "± 512727",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 404849,
            "range": "± 1481",
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
          "id": "b8de7c14f93e8de0797b82c1e0cb8f14987f34ef",
          "message": "Add look up table for FQN to cache",
          "timestamp": "2026-03-08T18:43:35+01:00",
          "tree_id": "06f0d8c7313bb3280b0ffe69f08f6531cdbb8020",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/b8de7c14f93e8de0797b82c1e0cb8f14987f34ef"
        },
        "date": 1772992263303,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29162,
            "range": "± 539",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 130615,
            "range": "± 1171",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 254699,
            "range": "± 3852",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 496537,
            "range": "± 2872",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 598928,
            "range": "± 5170",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2939067,
            "range": "± 26119",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5928015,
            "range": "± 32784",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 142154,
            "range": "± 1409",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 38117,
            "range": "± 278",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27162,
            "range": "± 209",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 43457,
            "range": "± 323",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 153146,
            "range": "± 1825",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 760043,
            "range": "± 7328",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2963527,
            "range": "± 10478",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 34880,
            "range": "± 151",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19351,
            "range": "± 157",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1702035,
            "range": "± 56192",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 35573,
            "range": "± 567",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 17749,
            "range": "± 78",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101156227,
            "range": "± 1987054",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 474448,
            "range": "± 2877",
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
          "id": "883bdc9d3910a0eb7a4f0be8c2408e71e135fae3",
          "message": "Clean up roadmap",
          "timestamp": "2026-03-08T19:19:00+01:00",
          "tree_id": "9d203148971a569aa6883b61b1c1f710c856ff65",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/883bdc9d3910a0eb7a4f0be8c2408e71e135fae3"
        },
        "date": 1772994250058,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29639,
            "range": "± 201",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 132848,
            "range": "± 923",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 256271,
            "range": "± 3056",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 496292,
            "range": "± 7335",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 603388,
            "range": "± 24196",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2941402,
            "range": "± 17739",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5967208,
            "range": "± 85361",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 145575,
            "range": "± 4268",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 37577,
            "range": "± 321",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27493,
            "range": "± 169",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 45397,
            "range": "± 326",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 152542,
            "range": "± 1578",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 764698,
            "range": "± 4379",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 3009775,
            "range": "± 56873",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35779,
            "range": "± 483",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 20329,
            "range": "± 89",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1693428,
            "range": "± 15237",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36021,
            "range": "± 496",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 17556,
            "range": "± 56",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101746644,
            "range": "± 586607",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 467660,
            "range": "± 10353",
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
          "id": "58ffe1c0cc1265ada80a8d515036d4377287555f",
          "message": "Provide upgrade actions for deprecated code",
          "timestamp": "2026-03-08T20:17:11+01:00",
          "tree_id": "e09ddce3ed0ac11defd511cbc5f18128e2c58182",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/58ffe1c0cc1265ada80a8d515036d4377287555f"
        },
        "date": 1772997746313,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29049,
            "range": "± 445",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 133853,
            "range": "± 1389",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 259218,
            "range": "± 3922",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 504024,
            "range": "± 4907",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 607569,
            "range": "± 4535",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 3028020,
            "range": "± 43270",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 6270568,
            "range": "± 493861",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 145344,
            "range": "± 1587",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 36898,
            "range": "± 290",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27124,
            "range": "± 211",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 44775,
            "range": "± 320",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 163489,
            "range": "± 1488",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 774404,
            "range": "± 4111",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 3056434,
            "range": "± 41365",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 34824,
            "range": "± 362",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19258,
            "range": "± 692",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1697917,
            "range": "± 28787",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36352,
            "range": "± 217",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 17961,
            "range": "± 69",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101683026,
            "range": "± 786745",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 467191,
            "range": "± 2463",
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
          "id": "3875faf9df73b15412147e6c7788eea01d96ec3e",
          "message": "Non-Composer project support.",
          "timestamp": "2026-03-09T00:11:31+01:00",
          "tree_id": "68cee3492780f48af804979ebe581efc068f00e2",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/3875faf9df73b15412147e6c7788eea01d96ec3e"
        },
        "date": 1773011807840,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 30124,
            "range": "± 184",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 133523,
            "range": "± 1044",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 260182,
            "range": "± 2744",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 507943,
            "range": "± 7919",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 606247,
            "range": "± 9955",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2876646,
            "range": "± 29059",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5736320,
            "range": "± 43515",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 147014,
            "range": "± 1469",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 37250,
            "range": "± 509",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27694,
            "range": "± 201",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 45700,
            "range": "± 245",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 165745,
            "range": "± 6728",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 770903,
            "range": "± 8974",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2990073,
            "range": "± 14469",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35324,
            "range": "± 187",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19826,
            "range": "± 89",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1700789,
            "range": "± 19152",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36907,
            "range": "± 228",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 18156,
            "range": "± 167",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100812002,
            "range": "± 404724",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 481499,
            "range": "± 3736",
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
          "id": "d4cdc2f2cd999c7f5d338a7c2ec0fc540cd325de",
          "message": "Indexing progress indicator",
          "timestamp": "2026-03-09T00:25:27+01:00",
          "tree_id": "b6cd123cc946d71c95cf1d8a64ee1a4647219a57",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/d4cdc2f2cd999c7f5d338a7c2ec0fc540cd325de"
        },
        "date": 1773012629418,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 25115,
            "range": "± 405",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 121217,
            "range": "± 629",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 245727,
            "range": "± 3098",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 488451,
            "range": "± 2705",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 599579,
            "range": "± 2888",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 3000513,
            "range": "± 10369",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 6037688,
            "range": "± 24223",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 127096,
            "range": "± 737",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 33972,
            "range": "± 627",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 25851,
            "range": "± 228",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 39280,
            "range": "± 312",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 162073,
            "range": "± 1068",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 775744,
            "range": "± 6654",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 3056770,
            "range": "± 15099",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 26532,
            "range": "± 139",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 16775,
            "range": "± 57",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1724938,
            "range": "± 14306",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 35323,
            "range": "± 188",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 18367,
            "range": "± 72",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 99027188,
            "range": "± 3379788",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 496478,
            "range": "± 9341",
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
          "id": "2d9c7035ed3614f7665c1e3bc9b2a40c82891459",
          "message": "Update requirements",
          "timestamp": "2026-03-09T00:46:19+01:00",
          "tree_id": "c0f76d5eee4739eba80a55441bc685c8e9d8956e",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/2d9c7035ed3614f7665c1e3bc9b2a40c82891459"
        },
        "date": 1773013888561,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29437,
            "range": "± 319",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 136248,
            "range": "± 1444",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 259615,
            "range": "± 1621",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 504036,
            "range": "± 3730",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 613101,
            "range": "± 9222",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2964450,
            "range": "± 131004",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5816550,
            "range": "± 38032",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 147687,
            "range": "± 860",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 37035,
            "range": "± 671",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27621,
            "range": "± 231",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 45810,
            "range": "± 253",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 163405,
            "range": "± 600",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 774145,
            "range": "± 3881",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2992469,
            "range": "± 22710",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35391,
            "range": "± 189",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19601,
            "range": "± 78",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1698908,
            "range": "± 25945",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36378,
            "range": "± 285",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 18093,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101143086,
            "range": "± 670021",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 468688,
            "range": "± 1900",
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
          "id": "a660ecfbd73729bd5d9c5176341d2d4d06e732a7",
          "message": "Fix various type issues",
          "timestamp": "2026-03-09T02:44:17+01:00",
          "tree_id": "829761a7ee52f1ea2cdb049b90414ea39c1ece56",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/a660ecfbd73729bd5d9c5176341d2d4d06e732a7"
        },
        "date": 1773020969693,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 30427,
            "range": "± 1143",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 137483,
            "range": "± 766",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 267441,
            "range": "± 6224",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 516930,
            "range": "± 4373",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 616552,
            "range": "± 3471",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2933755,
            "range": "± 29175",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 6108902,
            "range": "± 149910",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 149123,
            "range": "± 2046",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 38277,
            "range": "± 689",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27600,
            "range": "± 347",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 45261,
            "range": "± 306",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 165771,
            "range": "± 1073",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 780195,
            "range": "± 12767",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 3029007,
            "range": "± 75432",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35742,
            "range": "± 291",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19788,
            "range": "± 141",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1716447,
            "range": "± 13167",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36696,
            "range": "± 452",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 18310,
            "range": "± 180",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101872582,
            "range": "± 752276",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 481150,
            "range": "± 4643",
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
          "id": "e10d989f3767c760e9f51a08d29495f7b74026e0",
          "message": "Fix two types from PHPDoc issues",
          "timestamp": "2026-03-09T03:03:57+01:00",
          "tree_id": "fde2cc668994c9b7fb50e514c2beed84e5647f7c",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/e10d989f3767c760e9f51a08d29495f7b74026e0"
        },
        "date": 1773022145645,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29607,
            "range": "± 431",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 133848,
            "range": "± 1015",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 259502,
            "range": "± 2203",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 506946,
            "range": "± 10170",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 610445,
            "range": "± 4775",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2963617,
            "range": "± 24817",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5884882,
            "range": "± 44199",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 148513,
            "range": "± 1234",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 37716,
            "range": "± 397",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27714,
            "range": "± 506",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 45112,
            "range": "± 618",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 164089,
            "range": "± 2717",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 769791,
            "range": "± 5593",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2986072,
            "range": "± 15043",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 34994,
            "range": "± 519",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19446,
            "range": "± 122",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1691121,
            "range": "± 13649",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36932,
            "range": "± 177",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 18380,
            "range": "± 154",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 99535656,
            "range": "± 1618213",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 465928,
            "range": "± 3209",
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
          "id": "3b4f193bd75c65c89f53718feb9d58ec84306aad",
          "message": "Fix 8 bugs",
          "timestamp": "2026-03-09T06:09:54+01:00",
          "tree_id": "5aca8933cd2f5ea15654c002a33fb0c4bdd7d4a3",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/3b4f193bd75c65c89f53718feb9d58ec84306aad"
        },
        "date": 1773033305720,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 28947,
            "range": "± 716",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 135094,
            "range": "± 1801",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 258070,
            "range": "± 4575",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 500116,
            "range": "± 3703",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 612619,
            "range": "± 3767",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2970043,
            "range": "± 29472",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 8778255,
            "range": "± 311982",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 146380,
            "range": "± 949",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 38191,
            "range": "± 259",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27573,
            "range": "± 291",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 45742,
            "range": "± 401",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 164766,
            "range": "± 715",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 770460,
            "range": "± 7948",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 3014058,
            "range": "± 18338",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35467,
            "range": "± 348",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19309,
            "range": "± 112",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1703343,
            "range": "± 11162",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36149,
            "range": "± 234",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 17692,
            "range": "± 49",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100270158,
            "range": "± 1105530",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 468143,
            "range": "± 1818",
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
          "id": "f87335fd8b76397539573af7e5927ad59e3aa846",
          "message": "Fix additional type issues",
          "timestamp": "2026-03-09T06:44:28+01:00",
          "tree_id": "52529616c512a4afa7ae572fe79b86a91892a3ac",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/f87335fd8b76397539573af7e5927ad59e3aa846"
        },
        "date": 1773035381488,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29590,
            "range": "± 532",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 134842,
            "range": "± 4035",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 262167,
            "range": "± 3887",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 502415,
            "range": "± 4036",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 612041,
            "range": "± 6869",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2985189,
            "range": "± 63883",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 8507274,
            "range": "± 336966",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 147591,
            "range": "± 1207",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 38336,
            "range": "± 279",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27475,
            "range": "± 577",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 46214,
            "range": "± 3399",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 165535,
            "range": "± 1417",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 769645,
            "range": "± 4828",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 3020083,
            "range": "± 21864",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35431,
            "range": "± 219",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19530,
            "range": "± 138",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1712033,
            "range": "± 12432",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36117,
            "range": "± 140",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 17810,
            "range": "± 97",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100539400,
            "range": "± 2147511",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 470659,
            "range": "± 4076",
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
          "id": "5b864d625fce4044c2e90e1a36552757fe515997",
          "message": "Function-level `@template` with generic return types",
          "timestamp": "2026-03-09T21:40:57+01:00",
          "tree_id": "d2d6e0888080f25188e0aa65a89da11eed8a5045",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/5b864d625fce4044c2e90e1a36552757fe515997"
        },
        "date": 1773089175566,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29717,
            "range": "± 594",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 136530,
            "range": "± 916",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 261627,
            "range": "± 1484",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 508811,
            "range": "± 7101",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 620324,
            "range": "± 4116",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2973346,
            "range": "± 20703",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5940307,
            "range": "± 34582",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 149067,
            "range": "± 879",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 38194,
            "range": "± 486",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27489,
            "range": "± 244",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 45687,
            "range": "± 309",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 164120,
            "range": "± 893",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 772497,
            "range": "± 2688",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 2994429,
            "range": "± 25784",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35263,
            "range": "± 570",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19952,
            "range": "± 114",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1699092,
            "range": "± 13339",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36925,
            "range": "± 241",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 18275,
            "range": "± 118",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100464448,
            "range": "± 491080",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 470182,
            "range": "± 2568",
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
          "id": "415f4c9024a7e1f7e4ad710da3d1c359aa287df9",
          "message": "Fix several issues around resolving context inside closures",
          "timestamp": "2026-03-10T06:13:13+01:00",
          "tree_id": "ac606cee13d40d57c3f21b8c82076d19e5137027",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/415f4c9024a7e1f7e4ad710da3d1c359aa287df9"
        },
        "date": 1773119913337,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29114,
            "range": "± 2508",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 139414,
            "range": "± 9024",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 258426,
            "range": "± 2567",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 503370,
            "range": "± 30495",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 615453,
            "range": "± 51818",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2990813,
            "range": "± 500604",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 6311811,
            "range": "± 2226745",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 148525,
            "range": "± 35434",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 38304,
            "range": "± 5549",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27298,
            "range": "± 2624",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 44897,
            "range": "± 10888",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 166130,
            "range": "± 27322",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 786056,
            "range": "± 123477",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 3184272,
            "range": "± 1473152",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35323,
            "range": "± 9194",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19471,
            "range": "± 3081",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1741981,
            "range": "± 357016",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 36692,
            "range": "± 209",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 17791,
            "range": "± 63",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 100317575,
            "range": "± 724215",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 487127,
            "range": "± 7963",
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
          "id": "6631f803b9d7ef3cfb7b83d1573d3ded40ea3f7a",
          "message": "Closure parameter with parent type hint",
          "timestamp": "2026-03-10T18:18:22+01:00",
          "tree_id": "dfe4bfd3e2e47eb7208dca25ae7eb96b61c37526",
          "url": "https://github.com/AJenbo/phpantom_lsp/commit/6631f803b9d7ef3cfb7b83d1573d3ded40ea3f7a"
        },
        "date": 1773163419555,
        "tool": "cargo",
        "benches": [
          {
            "name": "completion_simple_class",
            "value": 29667,
            "range": "± 641",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_5",
            "value": 132368,
            "range": "± 1517",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_10",
            "value": 258600,
            "range": "± 3307",
            "unit": "ns/iter"
          },
          {
            "name": "completion_inheritance_depth/depth_20",
            "value": 503186,
            "range": "± 5090",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/100_classes",
            "value": 601127,
            "range": "± 4136",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/500_classes",
            "value": 2907907,
            "range": "± 25198",
            "unit": "ns/iter"
          },
          {
            "name": "completion_classmap_size/1000_classes",
            "value": 5944752,
            "range": "± 58752",
            "unit": "ns/iter"
          },
          {
            "name": "completion_generics_and_mixins",
            "value": 147837,
            "range": "± 792",
            "unit": "ns/iter"
          },
          {
            "name": "completion_with_narrowing",
            "value": 37491,
            "range": "± 231",
            "unit": "ns/iter"
          },
          {
            "name": "completion_5_method_chain",
            "value": 27385,
            "range": "± 516",
            "unit": "ns/iter"
          },
          {
            "name": "completion_cross_file_type_hint",
            "value": 45320,
            "range": "± 262",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/100_lines",
            "value": 165760,
            "range": "± 1403",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/500_lines",
            "value": 771710,
            "range": "± 4465",
            "unit": "ns/iter"
          },
          {
            "name": "update_ast_parse_time/2000_lines",
            "value": 3029556,
            "range": "± 15036",
            "unit": "ns/iter"
          },
          {
            "name": "hover_method_call",
            "value": 35194,
            "range": "± 624",
            "unit": "ns/iter"
          },
          {
            "name": "goto_definition_method",
            "value": 19670,
            "range": "± 100",
            "unit": "ns/iter"
          },
          {
            "name": "reparse_500_line_file",
            "value": 1704039,
            "range": "± 21779",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_generic_objects",
            "value": 35300,
            "range": "± 221",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_new_objects",
            "value": 17989,
            "range": "± 108",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/lots_of_missing_methods",
            "value": 101047554,
            "range": "± 2338492",
            "unit": "ns/iter"
          },
          {
            "name": "diagnostics/fixture/method_chain",
            "value": 468820,
            "range": "± 11425",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}