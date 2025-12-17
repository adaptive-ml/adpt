from adaptive_harmony.runtime import recipe_main, RecipeContext
from time import sleep


@recipe_main
async def main(ctx: RecipeContext):

    stage_one, stage_two, stage_three = ["Stage One", "Stage Two", "Stage Three"]
    ctx.job.register_stages([stage_one, stage_two, stage_three])

    ctx.job.report_progress(stage_one)

    total = 100
    for i in range(0, total + 1):
        ctx.job.report_progress(stage_one, total, i)
        sleep(1)

    ctx.job.report_progress(stage_two)

    total = 10
    for i in range(0, total + 1):
        ctx.job.report_progress(stage_two, total, i)
        sleep(10)

    ctx.job.report_progress(stage_three)

    sleep(30)
