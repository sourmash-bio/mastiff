rule bacteria_1k:
    output: directory("outputs/bacteria-1k")
    shell: """
        cargo run --release -- index -k 51 -s 1000 \
          --output {output} \
          <(head -1000 flist)
    """

rule bacteria_1k_save_paths:
    output: directory("outputs/bacteria-1k-save-paths")
    shell: """
        cargo run --release -- index -k 51 -s 1000 \
          --output {output} \
          --save-paths \
          <(head -1000 flist)
    """

rule bacteria_2k:
    output: directory("outputs/bacteria-2k")
    input: 
        previous=directory("outputs/bacteria-1k-save-paths"),
    shell: """
        cp -a {input.previous} {output}
        cargo run --release -- update -k 51 -s 1000 \
          --output {output} \
          --save-paths \
          <(head -2000 flist)
    """

rule bacteria_1k_colors:
    output: directory("outputs/bacteria-1k-colors")
    shell: """
        RUST_LOG=debug cargo run --release -- index -k 51 -s 1000 --colors \
          --output {output} \
          <(head -1000 flist)
    """

rule bacteria_10k:
    output: directory("outputs/bacteria-10k")
    shell: """
        cargo run --release -- index -k 51 -s 1000 \
          --output {output} \
          <(head -10000 flist)
    """

rule bacteria_10k_colors:
    output: directory("outputs/bacteria-10k-colors")
    shell: """
        cargo run --release -- index -k 51 -s 1000 --colors \
          --output {output} \
          <(head -10000 flist)
    """

rule bacteria_fullcolors:
    output: directory("outputs/bacteria-fullcolors")
    shell: """
        cargo run --release -- index -k 51 -s 1000 --colors \
          --output {output} \
          <(head -1 flist | xargs yes | head -1000)
    """

rule bacteria_k51_s1000:
    output: directory("outputs/bacteria-k51-s1000")
    benchmark:
        "benchmarks/bacteria-k51-s1000.tsv"
    shell: """
        cargo run --release -- index -k 51 -s 1000 \
          --output {output} \
          flist_bacteria-k51
    """


#rule metag_all:
#    output: directory("/scratch/analysis/metag_k21_s{scaled,\d+}-roaring")
#    params:
#        scaled = "{scaled}"
#    shell: """
#        cargo run --release -- index -k 21 -s {params.scaled} \
#            --output {output} \
#            --save-paths \
#            catalog_metagenomes
#    """

rule metag_all_update:
    input:
        updated="updated_catalog"
    params:
        scaled = "1000"
    shell: """
        cargo run --release -- update -k 21 -s {params.scaled} \
            --output /scratch/analysis/metag_k21_s{params.scaled}-roaring \
            --save-paths \
            {input.updated}
    """

rule metag_fullcolors:
    output: directory("outputs/fullcolors")
    shell: """
        cargo run --release -- index -k 21 -s 1000 \
            --output {output} \
            --colors \
            <(head -1 catalog_metagenomes | xargs yes | head -1000)
    """

rule metag_1k:
    output: directory("outputs/metag-1k")
    shell: """
        cargo run --release -- index -k 21 -s 1000 \
            --output {output} \
            --save-paths \
            <(head -1000 catalog_metagenomes)
    """

rule metag_1k_colors:
    output: directory("outputs/metag-1k-colors")
    shell: """
        RUST_LOG=trace cargo run --release -- index -k 21 -s 1000 \
            --output {output} \
            --colors \
            --save-paths \
            <(head -1000 catalog_metagenomes)
    """

rule download_rs_207:
    output: "inputs/gtdb-rs207.genomic-reps.dna.k21.zip"
    shell: """
        curl -L https://osf.io/download/f2wzc/ -o {output}
    """

rule extract_rs_207:
    output: 
        manifest="inputs/gtdb-rs207.genomic-reps.dna.k21.manifest",
        data=directory("inputs/gtdb-rs207.genomic-reps.dna.k21")
    input: "inputs/gtdb-rs207.genomic-reps.dna.k21.zip"
    shell: """
        python -m zipfile -e {input} {output.data}
        find `realpath {output.data}` -iname "*.sig.gz" > {output.manifest}
    """

rule rs_207:
    output: directory("outputs/rs207")
    input: 
      manifest="inputs/gtdb-rs207.genomic-reps.dna.k21.manifest",
      data=directory("inputs/gtdb-rs207.genomic-reps.dna.k21")
    shell: """
        RUST_LOG=trace cargo run --release -- index -k 21 -s 1000 \
            --output {output} \
            {input.manifest}
    """

rule new_catalog:
    output:
        new_catalog="updated_catalog",
    input:
        original="catalog_metagenomes",
        runinfo="runinfo.csv"
    run:
        import csv
        from pathlib import Path

        # load all sra IDs
        sraids = set()
        with open(input.runinfo) as fp:
            data = csv.DictReader(fp, delimiter=",")
            for dataset in data:
                if dataset['Run'] != 'Run':
                    sraids.add(dataset['Run'])

        with open(output.new_catalog, 'w') as out:
            # remove current datasets already in the catalog
            with open(input.original) as fp:
                for line in fp:
                    # write current line (to keep order)
                    out.write(line)

                    parts = line.strip().split('/')
                    sra_id = parts[-1].split('.')[0]
                    if sra_id in sraids:
                        sraids.remove(sra_id)

            path = Path("/".join(parts[:-1]))
            # check if remaining sraids exist on disk
            for sra_id in sraids:
                sig_path = path / f"{sra_id}.sig"
                if sig_path.exists():
                    out.write(f"{sig_path}\n")

"""
cargo run --release -- index -k 21 -s 1000 --output /scratch/analysis/rocksdb_metagenomes catalog_metagenomes                                                                               │ 
cargo run --release -- index -k 21 -s 10000 --output /scratch/analysis/rocksdb_metagenomes2 <(cat catalog_metagenomes | head 1000)                                                          │ 
cargo run --release -- index -k 21 -s 1000 --output /scratch/analysis/rocksdb_metagenomes2 <(cat catalog_metagenomes | head -n 10)                                                          │
cargo run --release -- index -k 21 -s 10000 --output /scratch/analysis/rocksdb_metagenomes2 <(cat catalog_metagenomes | head -n 10)                                                         │ 
cargo run --release -- index -k 21 -s 10000 --output /scratch/analysis/rocksdb_metagenomes2 <(cat catalog_metagenomes | head -n 1000)                                                       │
cargo run --release -- index -k 51 -s 1000 --output bacteria-100k-enum flist                                                                                                                │
cargo run --release -- index -k 51 -s 1000 --output bacteria-100k-cf <(head -1 flist)                                                                                                       │
cargo run --release -- index -k 51 -s 1000 --output bacteria-1k-cf (head -1000 flist)                                                                                                       │
cargo run --release -- index -k 51 -s 1000 --output bacteria-1k-cf $(head -1000 flist)                                                                                                      │
cargo run --release -- index -k 51 -s 1000 --output bacteria-10k-cf <(head -10000 flist)                                                                                                    │
cargo run --release -- index -k 51 -s 1000 --output bacteria-100k-cf flist                                                                                                                  │
cargo run --release -- index -k 51 -s 1000 --output bacteria-1k-cf-opts <(head -1000 flist)                                                                                                 │
cargo run --release -- index -k 51 -s 1000 --output bacteria-100k-cf-opts flist 
"""
