from __future__ import annotations

import json
from pathlib import Path

from docx import Document
from docx.enum.section import WD_SECTION
from docx.enum.table import WD_CELL_VERTICAL_ALIGNMENT, WD_TABLE_ALIGNMENT
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.shared import Cm, Pt, RGBColor


ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "competition" / "灵枢项目书实验报告.docx"


def set_cell_shading(cell, fill):
    tc_pr = cell._tc.get_or_add_tcPr()
    shd = tc_pr.find(qn("w:shd"))
    if shd is None:
        shd = OxmlElement("w:shd")
        tc_pr.append(shd)
    shd.set(qn("w:fill"), fill)


def set_cell_margins(cell, top=110, start=130, bottom=110, end=130):
    tc = cell._tc
    tc_pr = tc.get_or_add_tcPr()
    mar = tc_pr.first_child_found_in("w:tcMar")
    if mar is None:
        mar = OxmlElement("w:tcMar")
        tc_pr.append(mar)
    for name, value in (("top", top), ("start", start), ("bottom", bottom), ("end", end)):
        node = mar.find(qn(f"w:{name}"))
        if node is None:
            node = OxmlElement(f"w:{name}")
            mar.append(node)
        node.set(qn("w:w"), str(value))
        node.set(qn("w:type"), "dxa")


def set_repeat_table_header(row):
    tr_pr = row._tr.get_or_add_trPr()
    node = OxmlElement("w:tblHeader")
    node.set(qn("w:val"), "true")
    tr_pr.append(node)


def set_run_font(run, east="Microsoft YaHei", latin="Aptos", size=None, bold=None, color=None):
    run.font.name = latin
    run._element.get_or_add_rPr().rFonts.set(qn("w:eastAsia"), east)
    run._element.get_or_add_rPr().rFonts.set(qn("w:ascii"), latin)
    run._element.get_or_add_rPr().rFonts.set(qn("w:hAnsi"), latin)
    if size is not None:
        run.font.size = Pt(size)
    if bold is not None:
        run.bold = bold
    if color:
        run.font.color.rgb = RGBColor(*color)


def add_table(doc, headers, rows, widths=None, alignments=None):
    table = doc.add_table(rows=1, cols=len(headers))
    table.alignment = WD_TABLE_ALIGNMENT.CENTER
    table.autofit = False
    table.style = "Table Grid"
    hdr = table.rows[0]
    set_repeat_table_header(hdr)
    for i, text in enumerate(headers):
        cell = hdr.cells[i]
        cell.text = str(text)
        cell.vertical_alignment = WD_CELL_VERTICAL_ALIGNMENT.CENTER
        set_cell_shading(cell, "1F4E78")
        set_cell_margins(cell)
        if widths:
            cell.width = Cm(widths[i])
        for p in cell.paragraphs:
            p.alignment = WD_ALIGN_PARAGRAPH.CENTER
            for run in p.runs:
                set_run_font(run, size=9, bold=True, color=(255, 255, 255))
    for r_index, values in enumerate(rows):
        cells = table.add_row().cells
        for i, value in enumerate(values):
            cell = cells[i]
            cell.text = str(value)
            cell.vertical_alignment = WD_CELL_VERTICAL_ALIGNMENT.CENTER
            set_cell_margins(cell)
            if widths:
                cell.width = Cm(widths[i])
            if r_index % 2:
                set_cell_shading(cell, "F3F7FA")
            for p in cell.paragraphs:
                p.alignment = (alignments[i] if alignments else WD_ALIGN_PARAGRAPH.LEFT)
                p.paragraph_format.space_after = Pt(0)
                p.paragraph_format.line_spacing = 1.08
                for run in p.runs:
                    set_run_font(run, size=8.7)
    doc.add_paragraph().paragraph_format.space_after = Pt(0)
    return table


def add_body(doc, text, bold_lead=None):
    p = doc.add_paragraph()
    p.paragraph_format.first_line_indent = Cm(0.74)
    p.paragraph_format.line_spacing = 1.38
    p.paragraph_format.space_after = Pt(6)
    if bold_lead and text.startswith(bold_lead):
        lead = p.add_run(bold_lead)
        set_run_font(lead, size=10.5, bold=True)
        rest = p.add_run(text[len(bold_lead):])
        set_run_font(rest, size=10.5)
    else:
        run = p.add_run(text)
        set_run_font(run, size=10.5)
    return p


def add_bullet(doc, text):
    p = doc.add_paragraph(style="List Bullet")
    p.paragraph_format.left_indent = Cm(0.8)
    p.paragraph_format.first_line_indent = Cm(-0.35)
    p.paragraph_format.line_spacing = 1.25
    p.paragraph_format.space_after = Pt(3)
    set_run_font(p.add_run(text), size=10)


def configure_styles(doc):
    styles = doc.styles
    normal = styles["Normal"]
    normal.font.name = "Aptos"
    normal.font.size = Pt(10.5)
    normal._element.rPr.rFonts.set(qn("w:eastAsia"), "Microsoft YaHei")
    normal.paragraph_format.space_after = Pt(6)
    for name, size, before, after in (("Title", 24, 0, 12), ("Subtitle", 12, 0, 24),
                                      ("Heading 1", 16, 16, 8), ("Heading 2", 12.5, 12, 5)):
        style = styles[name]
        style.font.name = "Aptos Display" if name != "Normal" else "Aptos"
        style.font.size = Pt(size)
        style.font.bold = name != "Subtitle"
        style.font.color.rgb = RGBColor(0, 0, 0)
        style._element.rPr.rFonts.set(qn("w:eastAsia"), "Microsoft YaHei")
        style.paragraph_format.space_before = Pt(before)
        style.paragraph_format.space_after = Pt(after)
        style.paragraph_format.keep_with_next = True
    # Word/LibreOffice may add a theme border to the built-in Title style.
    title_ppr = styles["Title"]._element.get_or_add_pPr()
    borders = title_ppr.find(qn("w:pBdr"))
    if borders is not None:
        title_ppr.remove(borders)


def add_heading(doc, text, level=1):
    p = doc.add_heading(text, level=level)
    for run in p.runs:
        set_run_font(run, east="Microsoft YaHei", latin="Aptos Display",
                     size=16 if level == 1 else 12.5, bold=True, color=(0, 0, 0))
    return p


def add_page_number(paragraph):
    paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER
    run = paragraph.add_run()
    begin = OxmlElement("w:fldChar")
    begin.set(qn("w:fldCharType"), "begin")
    instruction = OxmlElement("w:instrText")
    instruction.set(qn("xml:space"), "preserve")
    instruction.text = " PAGE "
    separate = OxmlElement("w:fldChar")
    separate.set(qn("w:fldCharType"), "separate")
    value = OxmlElement("w:t")
    value.text = "1"
    end = OxmlElement("w:fldChar")
    end.set(qn("w:fldCharType"), "end")
    for node in (begin, instruction, separate, value, end):
        run._r.append(node)
    set_run_font(run, size=8, color=(100, 100, 100))


def main():
    eval_data = json.loads((ROOT / "output" / "eval_results.json").read_text(encoding="utf-8"))
    qa = json.loads((ROOT / "output" / "locomo_qa_v2_300.json").read_text(encoding="utf-8"))
    retrieval = json.loads((ROOT / "output" / "retrieval_v2.json").read_text(encoding="utf-8"))
    zte_test = json.loads((ROOT / "output" / "zte_v3_test_results.json").read_text(encoding="utf-8"))

    doc = Document()
    section = doc.sections[0]
    section.top_margin = Cm(2.2)
    section.bottom_margin = Cm(2.0)
    section.left_margin = Cm(2.2)
    section.right_margin = Cm(2.2)
    configure_styles(doc)
    add_page_number(section.footer.paragraphs[0])

    title = doc.add_paragraph(style="Title")
    title.alignment = WD_ALIGN_PARAGRAPH.CENTER
    title_ppr = title._p.get_or_add_pPr()
    borders = title_ppr.find(qn("w:pBdr"))
    if borders is not None:
        title_ppr.remove(borders)
    set_run_font(title.add_run("灵枢动态记忆系统实验报告"), size=24, bold=True)
    subtitle = doc.add_paragraph(style="Subtitle")
    subtitle.alignment = WD_ALIGN_PARAGRAPH.CENTER
    set_run_font(subtitle.add_run("中国国际大学生创新大赛产业赛道 中兴通讯第 1529 题"), size=12)
    meta = doc.add_paragraph()
    meta.alignment = WD_ALIGN_PARAGRAPH.CENTER
    meta.paragraph_format.space_after = Pt(24)
    set_run_font(meta.add_run("原型名称  灵枢 Memory Evolution Lab\n实验日期  2026年9月6日"), size=10, color=(85, 85, 85))

    add_heading(doc, "报告摘要", 1)
    add_body(doc, "本报告验证灵枢在长时序记忆、公开长对话问答、过期与冲突治理、多智能体权限以及端侧检索性能方面的阶段性能力。核心结果显示：在包含660轮记忆和300条查询的自建回归集上，系统测试集Top-1准确率为94%，静态RAG基线为62%，提升32个百分点；在LoCoMo公开数据的278题类别分层样本上，问答得分由27.45提升至34.88；在10万条记忆规模下，单查询P99时延为195.19毫秒。")
    add_body(doc, "上述结果支持项目书中关于长时序记忆准确率、相对基线提升和端侧检索时延的量化论证。标准Precision@5、完整开放域任务完成度、百万级并发和真实多智能体协作尚未形成充分证据，因此本报告不作达标声明。")

    add_heading(doc, "核心实验结果", 1)
    add_table(doc,
              ["实验项目", "数据规模", "静态基线", "灵枢结果", "变化"],
              [["长时序Top-1准确率", "测试集100查询", "62.00%", "94.00%", "+32.00pp"],
               ["事实变更Top-1准确率", "测试集50查询", "30.00%", "96.00%", "+66.00pp"],
               ["LoCoMo证据召回R@5", "公开集1,977 QA", "26.82%", "52.31%", "+25.49pp"],
               ["LoCoMo问答得分", "分层样本278题", "27.45", "34.88", "+7.43分"],
               ["检索P99时延", "10万条记忆 64次查询", "—", "195.19ms", "低于600ms"]],
              widths=[4.8, 4.0, 2.8, 2.8, 2.7],
              alignments=[WD_ALIGN_PARAGRAPH.LEFT, WD_ALIGN_PARAGRAPH.CENTER,
                          WD_ALIGN_PARAGRAPH.CENTER, WD_ALIGN_PARAGRAPH.CENTER, WD_ALIGN_PARAGRAPH.CENTER])

    doc.add_page_break()
    add_heading(doc, "一 实验目标与评价口径", 1)
    add_body(doc, "实验围绕命题要求建立静态RAG与灵枢检索的同数据对照，考察当前事实能否在长时序、事实变更、冗余与过期信息共存的条件下被正确检索，同时记录上下文开销、查询时延和内存占用。")
    add_table(doc, ["命题关注项", "采用指标", "当前证据状态"],
              [["长时序任务记忆准确率", "Top-1准确率及相对静态基线提升", "94% 对 62%，可用于项目书"],
               ["记忆检索Top-K质量", "标准P@5、R@5、Hit@5、MRR", "R@5/Hit@5可用；标准P@5未达到83%"],
               ["冗余与过期记忆过滤", "规则治理案例通过率", "18案例功能验证，可作为阶段性数据"],
               ["复杂任务完成度", "任务成功率", "3任务对照，样本较小"],
               ["Token消耗", "平均上下文Token及下降比例", "历史不同K对照可用，当前同预算未证明下降"],
               ["端侧轻量化", "P99时延与RSS", "10万规模结果可用"]],
              widths=[5.0, 5.4, 6.7],
              alignments=[WD_ALIGN_PARAGRAPH.LEFT, WD_ALIGN_PARAGRAPH.LEFT, WD_ALIGN_PARAGRAPH.LEFT])

    add_heading(doc, "二 数据集与实验设置", 1)
    add_heading(doc, "2.1 自建长时序回归集", 2)
    add_body(doc, "LingShu ZTE Synthetic v3包含15个项目实体、20类属性、300个不同项目属性键和300条查询。其中150个事实键经历两次事实变更，形成300条被替代事实；另含300条当前事实和60条过期或重复干扰记忆，共660轮。数据按项目实体划分为200条开发查询和100条测试查询，避免同一项目进入两个集合。")
    add_bullet(doc, "测试集：100条查询，其中50条为事实变更查询，50条为稳定事实查询。")
    add_bullet(doc, "对照条件：所有检索方案统一使用Top-5；Top-1用于长时序准确率。")
    add_bullet(doc, "数据属性：合成回归数据，状态标签由数据构造器提供，适合验证检索与过滤后的结果，不用于证明自动状态识别能力。")

    add_heading(doc, "2.2 LoCoMo公开长对话数据", 2)
    add_body(doc, "公开实验采用LoCoMo长对话数据的证据可评分部分，共10组对话、5,882条记忆和1,977个带证据问答。检索使用冻结的384维MiniLM向量；BM25参数为k1等于1.2、b等于0.75。所有检索方案统一返回Top-5。")
    add_body(doc, "端到端问答采用最后4组对话中的类别分层样本，共278题：多跳、时序、单跳和对抗题各60题，开放域题38题。生成模型为本地llama3.1:8b，temperature为0，随机种子为42，上下文窗口4096，最多生成128 token。问答评分采用LoCoMo类别规则：普通题使用词元F1，多跳题按子答案计算，对抗题计算拒答正确率。")

    add_heading(doc, "2.3 工程性能设置", 2)
    add_body(doc, "离线性能探针分别在1千、1万和10万条记忆规模下执行64次查询，记录平均时延与P50、P95、P99。内存对照使用10万条静态记忆和过滤后33,333条保留记忆的独立进程RSS。该结果反映检索内核性能，不等同于包含网络、数据库和模型生成的完整服务端到端时延。")

    add_heading(doc, "三 自建集实验结果", 1)
    add_heading(doc, "3.1 长时序检索准确率", 2)
    add_table(doc, ["方案", "全量Top-1 300题", "开发Top-1 200题", "测试Top-1 100题", "测试事实变更 50题"],
              [["Static RAG", "64.33%", "65.50%", "62.00%", "30.00%"],
               ["LingShu Full", "91.67%", "91.00%", "94.00%", "96.00%"],
               ["去除时间衰减", "92.00%", "91.50%", "94.00%", "96.00%"],
               ["去除来源可信度", "93.67%", "93.50%", "94.00%", "92.00%"],
               ["去除交互频次", "94.00%", "94.00%", "94.00%", "96.00%"],
               ["去除重要性", "95.00%", "93.50%", "98.00%", "100.00%"],
               ["相似度加状态过滤", "99.00%", "99.00%", "99.00%", "100.00%"]],
              widths=[4.3, 3.2, 3.2, 3.2, 3.3],
              alignments=[WD_ALIGN_PARAGRAPH.LEFT] + [WD_ALIGN_PARAGRAPH.CENTER] * 4)
    add_body(doc, "测试集上，灵枢完整模型Top-1准确率达到94%，较Static RAG的62%提高32个百分点；事实变更子集达到96%，较Static RAG的30%提高66个百分点。该结果满足命题提出的准确率不低于86%、相对静态基线提升不低于15个百分点的数值要求。")
    add_body(doc, "消融显示固定多特征权重不是每种分布下的最优解：去除重要性或仅保留相似度与状态过滤取得更高结果。因此，项目书可使用完整模型的94%作为当前系统结果，同时将权重按任务自适应列为后续优化方向。")

    add_heading(doc, "3.2 Top-K指标说明", 2)
    add_body(doc, "测试集每个查询只有一个相关证据，因此标准Precision@5的理论上限为20%。完整模型的P@5为19.80%，R@5和Hit@5均为99%。这些数值表明正确证据几乎都进入前5名，但不能据此声称Precision@5达到83%。若项目书需要验证该阈值，应新增每题包含多个相关证据的评测集，或取得命题方对Top-K指标定义的书面确认。")

    add_heading(doc, "四 公开数据集实验结果", 1)
    add_heading(doc, "4.1 LoCoMo证据检索", 2)
    test = retrieval["summary"]["test"]["arms"]
    allm = retrieval["summary"]["all"]["arms"]
    add_table(doc, ["检索方案", "全量R@5", "测试R@5", "测试P@5", "测试Top-1"],
              [["Static向量", f"{allm['static']['recall']*100:.2f}%", f"{test['static']['recall']*100:.2f}%", f"{test['static']['precision']*100:.2f}%", f"{test['static']['accuracy']*100:.2f}%"],
               ["BM25仅正文", f"{allm['bm25_text']['recall']*100:.2f}%", f"{test['bm25_text']['recall']*100:.2f}%", f"{test['bm25_text']['precision']*100:.2f}%", f"{test['bm25_text']['accuracy']*100:.2f}%"],
               ["BM25正文加说话人", f"{allm['bm25']['recall']*100:.2f}%", f"{test['bm25']['recall']*100:.2f}%", f"{test['bm25']['precision']*100:.2f}%", f"{test['bm25']['accuracy']*100:.2f}%"],
               ["向量加BM25融合", f"{allm['hybrid']['recall']*100:.2f}%", f"{test['hybrid']['recall']*100:.2f}%", f"{test['hybrid']['precision']*100:.2f}%", f"{test['hybrid']['accuracy']*100:.2f}%"],
               ["句子事实候选加向量", f"{allm['fact_hybrid']['recall']*100:.2f}%", f"{test['fact_hybrid']['recall']*100:.2f}%", f"{test['fact_hybrid']['precision']*100:.2f}%", f"{test['fact_hybrid']['accuracy']*100:.2f}%"]],
              widths=[4.8, 3.1, 3.1, 3.0, 3.1], alignments=[WD_ALIGN_PARAGRAPH.LEFT] + [WD_ALIGN_PARAGRAPH.CENTER] * 4)
    delta = retrieval["summary"]["test"]["paired_recall"]["bm25"]
    add_body(doc, f"在回溯式测试部分，BM25的R@5为51.82%，较Static向量的25.15%提高26.67个百分点。按4个对话簇进行2,000次配对bootstrap，提升的95%区间为{delta['cluster_bootstrap_95'][0]*100:.2f}至{delta['cluster_bootstrap_95'][1]*100:.2f}个百分点。由于测试仅含4个对话簇，该区间作为阶段性公开集证据。")

    add_heading(doc, "4.2 LoCoMo端到端问答", 2)
    q = qa["summary"]
    add_table(doc, ["问题类型", "样本数", "Static得分", "BM25得分", "变化"],
              [["总体", q["all"]["n"], f"{q['all']['static']*100:.2f}", f"{q['all']['bm25']*100:.2f}", f"+{(q['all']['bm25']-q['all']['static'])*100:.2f}"],
               ["多跳", q["1"]["n"], f"{q['1']['static']*100:.2f}", f"{q['1']['bm25']*100:.2f}", f"{(q['1']['bm25']-q['1']['static'])*100:+.2f}"],
               ["时序", q["2"]["n"], f"{q['2']['static']*100:.2f}", f"{q['2']['bm25']*100:.2f}", f"{(q['2']['bm25']-q['2']['static'])*100:+.2f}"],
               ["开放域", q["3"]["n"], f"{q['3']['static']*100:.2f}", f"{q['3']['bm25']*100:.2f}", f"{(q['3']['bm25']-q['3']['static'])*100:+.2f}"],
               ["单跳", q["4"]["n"], f"{q['4']['static']*100:.2f}", f"{q['4']['bm25']*100:.2f}", f"{(q['4']['bm25']-q['4']['static'])*100:+.2f}"],
               ["对抗拒答准确率", q["5"]["n"], f"{q['5']['static']*100:.2f}", f"{q['5']['bm25']*100:.2f}", f"{(q['5']['bm25']-q['5']['static'])*100:+.2f}"]],
              widths=[4.5, 2.4, 3.4, 3.4, 2.9], alignments=[WD_ALIGN_PARAGRAPH.LEFT] + [WD_ALIGN_PARAGRAPH.CENTER] * 4)
    ci = qa["paired_f1"]
    add_body(doc, f"278题类别分层样本的总体得分由27.45提升至34.88，增加7.43分；按对话簇配对bootstrap的95%区间为{ci['cluster_bootstrap_95'][0]*100:.2f}至{ci['cluster_bootstrap_95'][1]*100:.2f}分。该总体分数混合了四类词元F1和对抗拒答准确率，用于同协议下的方案比较，不作为LoCoMo全量官方成绩。")

    add_heading(doc, "五 治理 任务与多智能体功能验证", 1)
    gov = eval_data["governance"]
    tasks_s, tasks_a = eval_data["task_static"], eval_data["task_adaptive"]
    add_table(doc, ["验证项目", "基线或总数", "灵枢结果", "说明"],
              [["过期记忆过滤", "治理案例18个", f"{gov['stale_filter_rate']:.0f}%", "规则功能验证"],
               ["重复记忆过滤", "治理案例18个", f"{gov['duplicate_filter_rate']:.0f}%", "规则功能验证"],
               ["冲突解决率", "治理案例18个", f"{gov['conflict_resolution_rate']:.0f}%", "规则功能验证"],
               ["错误淘汰率", "治理案例18个", f"{gov['wrong_elimination_rate']:.0f}%", "小样本，不外推"],
               ["任务成功率", f"Static {tasks_s['task_success_rate']:.2f}%", f"{tasks_a['task_success_rate']:.2f}%", "3个开放域任务"],
               ["多Agent权限", "6项检查", f"{eval_data['multi_agent']['pass_rate']:.0f}%", "权限与审计功能测试"]],
              widths=[4.2, 4.1, 3.2, 5.0], alignments=[WD_ALIGN_PARAGRAPH.LEFT, WD_ALIGN_PARAGRAPH.CENTER,
                                                     WD_ALIGN_PARAGRAPH.CENTER, WD_ALIGN_PARAGRAPH.LEFT])
    add_body(doc, "治理、多智能体和开放域任务结果证明相应功能链路能够运行，但样本规模较小。项目书中宜表述为功能验证或阶段性实验，不将其描述为大规模统计结论。")

    add_heading(doc, "六 工程性能实验", 1)
    latency_rows = []
    for row in eval_data["latency"]:
        latency_rows.append([f"{row['memory_count']:,}", 64, f"{row['mean_ms']:.2f}ms", f"{row['p50_ms']:.2f}ms", f"{row['p95_ms']:.2f}ms", f"{row['p99_ms']:.2f}ms"])
    add_table(doc, ["记忆规模", "查询数", "平均", "P50", "P95", "P99"], latency_rows,
              widths=[3.4, 2.4, 3.0, 3.0, 3.0, 3.0], alignments=[WD_ALIGN_PARAGRAPH.CENTER] * 6)
    rss_static = next(x["rss_bytes"] for x in eval_data["rss"] if x["scenario"] == "static")
    rss_opt = next(x["rss_bytes"] for x in eval_data["rss"] if x["scenario"] == "optimized")
    rss_reduction = (rss_static - rss_opt) / rss_static * 100
    add_table(doc, ["场景", "记忆条数", "RSS", "变化"],
              [["静态全量", "100,000", f"{rss_static/1_000_000:.2f}MB", "—"],
               ["过滤后保留", "33,333", f"{rss_opt/1_000_000:.2f}MB", f"降低{rss_reduction:.2f}%"]],
              widths=[5.2, 3.8, 3.8, 4.4], alignments=[WD_ALIGN_PARAGRAPH.LEFT] + [WD_ALIGN_PARAGRAPH.CENTER] * 3)
    add_body(doc, "10万条规模的P99为195.19毫秒，低于命题提出的600毫秒端侧时延目标。RSS由165.37MB降至63.21MB，下降61.78%；该下降包含记忆保留数量由10万减少至33,333条的影响，反映过滤后工作集的内存收益。")

    add_heading(doc, "七 项目书可采用的结论", 1)
    add_body(doc, "建议在项目书正文中采用以下表述：")
    add_body(doc, "在包含660轮记忆和300条查询的自建长时序回归集上，灵枢完整模型在独立项目测试集中的Top-1准确率达到94%，相比Static RAG基线的62%提升32个百分点；在50条事实变更查询上达到96%，较基线提高66个百分点。在LoCoMo公开长对话数据的1,977个带证据问答上，Top-5证据召回率由26.82%提升至52.31%；在278题类别分层问答实验中，得分由27.45提升至34.88。在10万条记忆规模下，离线检索P99时延为195.19毫秒。", bold_lead="在包含660轮记忆和300条查询的自建长时序回归集上，")

    add_heading(doc, "八 结论边界与后续工作", 1)
    add_bullet(doc, "自建v3属于合成回归集，适合证明当前检索实现对预标注状态的利用效果；自动状态识别需另行评测。")
    add_bullet(doc, "LoCoMo测试划分为回溯式划分，新增BM25属于离线实验方案，尚未接入在线生产检索链路。")
    add_bullet(doc, "标准P@5当前为19.80%，不应写成达到83%；单一相关证据题的标准P@5上限为20%。")
    add_bullet(doc, "Token下降57.83%来自Static K等于12与Adaptive K等于5的历史对照；同Top-K条件下尚未证明Token下降，因此不列为本报告核心达标结论。")
    add_bullet(doc, "百万级规模、真实服务并发QPS和真实多Agent协作任务仍需补充。")

    add_heading(doc, "附录 复现命令与证据文件", 1)
    for command in [
        "python eval/build_zte_v3.py",
        "python eval/retrieval_experiment.py --out output/retrieval_v2.json",
        "python eval/locomo_qa_v2.py --per-cat 60 --out output/locomo_qa_v2_300.json",
        "cargo run -p lingshu-eval --release -- --zte-dataset datasets/zte_memory_v3_test.json --json output/zte_v3_test_results.json",
        "cargo test -p lingshu-eval",
    ]:
        p = doc.add_paragraph()
        p.paragraph_format.left_indent = Cm(0.5)
        p.paragraph_format.space_after = Pt(3)
        run = p.add_run(command)
        set_run_font(run, east="Microsoft YaHei", latin="Consolas", size=8.5, color=(45, 45, 45))
    add_body(doc, "主要机器可读证据：output/eval_results.json、output/retrieval_v2.json、output/locomo_qa_v2_300.json、output/zte_v3_test_results.json，以及docs/competition/evidence-v2.json。")

    core_props = doc.core_properties
    core_props.title = "灵枢动态记忆系统实验报告"
    core_props.subject = "中兴通讯第1529题项目书实验数据"
    core_props.author = "灵枢项目组"
    core_props.keywords = "动态记忆, 自适应检索, LoCoMo, 长时序记忆"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    doc.save(OUT)
    print(OUT)


if __name__ == "__main__":
    main()
